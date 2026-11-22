use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use e621_cruiser::api::{parse_search_posts_response, Pool, Post, NON_ARTIST_TAGS};
use e621_cruiser::blacklist::Blacklist;
use e621_cruiser::downloader::{format_tags_txt, make_download_jobs};
use e621_cruiser::settings::{compose_query, Settings};
use e621_cruiser::storage::{
    build_target, sanitize, DownloadJob, History, ImageCache, SpankBank,
};
use e621_cruiser::tags::TagAutocompleteState;
use e621_cruiser::ui::first_run_splash::show_first_run_splash;
use e621_cruiser::ui::state::{FirstRunSplashState, ResultTab, Source, SourceKind, ViewerState};
use e621_cruiser::ui::theme;
use e621_cruiser::workers::fetch_image_with_fetcher;
use serde_json::json;

fn make_post_json(id: u64) -> serde_json::Value {
    json!({
        "id": id,
        "file": {
            "md5": "abcdef",
            "ext": "png",
            "url": "https://x/abcdef.png",
            "width": 10,
            "height": 10,
            "size": 1
        },
        "preview": {"url": "https://x/p.jpg"},
        "sample": {"has": false},
        "score": {"total": 5},
        "rating": "e",
        "tags": {
            "artist": ["conditional_dnp", "someartist"],
            "general": ["solo", "smile"],
            "species": ["wolf"],
            "copyright": [],
            "character": []
        },
        "pools": []
    })
}

fn make_post(id: u64) -> Post {
    Post::from_api_json(&make_post_json(id)).unwrap()
}

#[test]
fn test_blacklist_rules() {
    let p = make_post(100);
    assert!(Blacklist::new("wolf").is_blocked(&p));
    assert!(!Blacklist::new("fox").is_blocked(&p));
    assert!(Blacklist::new("wolf solo").is_blocked(&p));
    assert!(!Blacklist::new("wolf -rating:e").is_blocked(&p));
    assert!(Blacklist::new("solo rating:e").is_blocked(&p));
    assert!(Blacklist::new("solo rating:explicit").is_blocked(&p));
    assert!(Blacklist::new("~fox ~wolf").is_blocked(&p));
    assert!(!Blacklist::new("# wolf\n-solo").is_blocked(&p));
    assert!(Blacklist::new("type:png").is_blocked(&p));
    assert!(Blacklist::new("id:100").is_blocked(&p));
    assert!(!Blacklist::new("id:101").is_blocked(&p));

    let p2 = make_post(101);
    let (kept, hidden) = Blacklist::new("id:100").filter(&[p, p2]);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].id, 101);
    assert_eq!(hidden, 1);
}

#[test]
fn test_organize_modes_and_build_target() {
    let p = make_post(100);
    let job = |mode: &str| DownloadJob::new(p.clone(), "/dl", mode);

    assert_eq!(
        build_target(&job("artist"), |_| None),
        PathBuf::from("/dl/artists/someartist/100.png")
    );
    assert_eq!(
        build_target(&job("species"), |_| None),
        PathBuf::from("/dl/species/wolf/100.png")
    );
    assert_eq!(
        build_target(&job("copyright"), |_| None),
        PathBuf::from("/dl/copyrights/_unknown_copyright/100.png")
    );
    assert_eq!(
        build_target(&job("character"), |_| None),
        PathBuf::from("/dl/characters/_unknown_character/100.png")
    );

    let mut search_job = job("search");
    search_job.source = "wolf solo".to_string();
    assert_eq!(
        build_target(&search_job, |_| None),
        PathBuf::from("/dl/searches/wolf solo/100.png")
    );
    assert_eq!(
        build_target(&job("flat"), |_| None),
        PathBuf::from("/dl/100.png")
    );
    assert_eq!(
        build_target(&job("rating"), |_| None),
        PathBuf::from("/dl/ratings/explicit/100.png")
    );

    let mut pool_job = job("search");
    pool_job.pool_id = Some(7);
    pool_job.pool_name = Some("My Comic".to_string());
    pool_job.pool_index = Some(3);
    assert_eq!(
        build_target(&pool_job, |_| None),
        PathBuf::from("/dl/pools/My Comic (7)/0003_100.png")
    );

    let mut post_in_pool = make_post(100);
    post_in_pool.pools = vec![9];
    let pool = Pool {
        id: 9,
        name: "Some_Comic".to_string(),
        post_ids: vec![50, 100],
        ..Default::default()
    };
    let j_pool = DownloadJob::new(post_in_pool, "/dl", "pool");
    assert_eq!(
        build_target(&j_pool, |_| Some(pool.clone())),
        PathBuf::from("/dl/pools/Some Comic (9)/0002_100.png")
    );

    let mut solo_post = make_post(100);
    solo_post.pools.clear();
    solo_post.tags.insert("artist".to_string(), vec!["fluffy_artist".to_string()]);
    assert_eq!(
        build_target(&DownloadJob::new(solo_post, "/dl", "pool"), |_| None),
        PathBuf::from("/dl/artists/fluffy_artist/100.png")
    );
}

#[test]
fn test_sanitize() {
    assert_eq!(sanitize(r#"a/b:c*?"<>|"#, 120), "a_b_c______");
    assert_eq!(sanitize("...", 120), "_");
    assert_eq!(sanitize("con", 120), "_con_");
    assert_eq!(sanitize("AUX", 120), "_AUX_");
    assert_eq!(sanitize("abcdef", 3), "abc");
}

#[test]
fn test_post_parsing_v1_v2_and_artists() {
    let p = make_post(100);
    assert_eq!(p.artists(), vec!["someartist".to_string()]);
    assert!(NON_ARTIST_TAGS.contains(&"conditional_dnp"));

    let mut missing = make_post_json(101);
    missing["file"]["url"] = json!(null);
    let p_missing = Post::from_api_json(&missing).unwrap();
    assert!(p_missing.file_url.is_none());

    let v2_raw = json!({
        "id": 734685,
        "files": {
            "meta": {"md5": "7dc945", "ext": "png", "size": 254896, "has_sample": true},
            "original": {"width": 2000, "height": 1500, "url": "https://static1.e621.net/data/7d/c9/7dc945.png"},
            "preview": {
                "width": 374,
                "height": 256,
                "jpg": "https://static1.e621.net/data/preview/7d/c9/7dc945.jpg",
                "webp": "https://static1.e621.net/data/preview/7d/c9/7dc945.webp"
            },
            "sample": {
                "width": 850,
                "height": 638,
                "jpg": "https://static1.e621.net/data/sample/7d/c9/7dc945.jpg",
                "webp": "https://static1.e621.net/data/sample/7d/c9/7dc945.webp"
            }
        },
        "stats": {
            "score": {"up": 80, "down": -2, "total": 78},
            "fav_count": 134,
            "is_favorited": true,
            "vote": 1
        },
        "has": {"sample": true},
        "pools": [6894],
        "rating": "e",
        "description": "hello",
        "tags": {
            "artist": ["conditional_dnp", "etuix"],
            "general": ["anthro", "comic"],
            "species": ["canine"]
        }
    });
    let pv2 = Post::from_api_json(&v2_raw).unwrap();
    assert_eq!(pv2.id, 734685);
    assert_eq!(pv2.md5, "7dc945");
    assert_eq!(pv2.ext, "png");
    assert_eq!(pv2.width, 2000);
    assert_eq!(pv2.height, 1500);
    assert_eq!(pv2.size, 254896);
    assert_eq!(
        pv2.file_url.as_deref(),
        Some("https://static1.e621.net/data/7d/c9/7dc945.png")
    );
    assert_eq!(
        pv2.preview_url.as_deref(),
        Some("https://static1.e621.net/data/preview/7d/c9/7dc945.jpg")
    );
    assert_eq!(
        pv2.sample_url.as_deref(),
        Some("https://static1.e621.net/data/sample/7d/c9/7dc945.jpg")
    );
    assert_eq!(pv2.score, 78);
    assert_eq!(pv2.fav_count, 134);
    assert!(pv2.is_favorited);
    assert_eq!(pv2.own_vote, 1);
    assert_eq!(pv2.artists(), vec!["etuix".to_string()]);
    assert_eq!(pv2.pools, vec![6894]);
    assert!(pv2.all_tags().contains("anthro"));

    let v2_flat = json!({
        "id": 999,
        "files": {
            "meta": {"md5": "abc", "ext": "jpg", "size": 100, "has_sample": false},
            "original": {"width": 500, "height": 500, "url": "https://x/orig.jpg"},
            "preview": {"width": 150, "height": 150, "jpg": null, "webp": "https://x/prev.webp"},
            "sample": {"width": 500, "height": 500, "jpg": "https://x/orig.jpg"}
        },
        "stats": {"score": {"total": 3}, "fav_count": 1, "is_favorited": false, "vote": 0},
        "rating": "s",
        "tags": ["wolf", "solo"]
    });
    let pflat = Post::from_api_json(&v2_flat).unwrap();
    assert_eq!(pflat.sample_url.as_deref(), Some("https://x/orig.jpg"));
    assert_eq!(pflat.preview_url.as_deref(), Some("https://x/prev.webp"));
    assert_eq!(
        pflat.tags.get("general").cloned(),
        Some(vec!["wolf".to_string(), "solo".to_string()])
    );

    let v2_arr_resp = json!([v2_raw]);
    let parsed_v2 = parse_search_posts_response(&v2_arr_resp);
    assert_eq!(parsed_v2.len(), 1);
    assert_eq!(parsed_v2[0].id, 734685);

    let legacy_resp = json!({
        "posts": [{
            "id": 2,
            "file": {"md5": "b", "ext": "jpg", "url": "https://x/2.jpg", "width": 20, "height": 20, "size": 2},
            "preview": {"url": "https://x/2p.jpg"},
            "sample": {"has": false},
            "score": {"total": 4},
            "rating": "q",
            "tags": {"artist": ["someone"]}
        }]
    });
    let parsed_legacy = parse_search_posts_response(&legacy_resp);
    assert_eq!(parsed_legacy.len(), 1);
    assert_eq!(parsed_legacy[0].id, 2);

    let pool = Pool::from_api_json(&json!({
        "id": 42,
        "name": "Cool_Comic",
        "post_ids": [1, 2, 3],
        "category": "series"
    }))
    .unwrap();
    assert_eq!(pool.display_name(), "Cool Comic");
    assert_eq!(pool.post_count(), 3);
    assert!(pool.is_active);
}

#[test]
fn test_settings_roundtrip_python_compat_and_compose_query() {
    let dir = tempfile::tempdir().unwrap();
    let cfg_path = dir.path().join("settings.json");

    // Simulate an existing Python-generated settings.json
    let python_json = json!({
        "username": "foxuser",
        "api_key": "secret123",
        "blacklist": "feral\ncomic",
        "download_dir": "/tmp/dl",
        "organize_mode": "artist",
        "write_tags_file": true,
        "skip_downloaded": true,
        "concurrent_downloads": 4,
        "rating_s": true,
        "rating_q": true,
        "rating_e": false,
        "min_score": 50,
        "order": "order:score",
        "media_filter": "-animated",
        "pool_only": true,
        "quick_blacklist": ["gore", "young"],
        "expand_blacklist": true,
        "expand_saved": false,
        "saved_searches": ["wolf solo"],
        "favorites_user": "foxuser",
        "window_geometry": "",
        "splitter_state": "",
        "slideshow_interval": 2.5
    });
    fs::write(&cfg_path, serde_json::to_string_pretty(&python_json).unwrap()).unwrap();

    let s = Settings::load_from(&cfg_path);
    assert_eq!(s.username, "foxuser");
    assert_eq!(s.quick_blacklist, vec!["gore", "young"]);
    assert!((s.slideshow_interval - 2.5).abs() < 1e-6);

    let fb = s.full_blacklist();
    assert!(fb.contains("feral"));
    assert!(fb.contains("comic"));
    assert!(fb.contains("gore"));
    assert!(fb.contains("young"));

    s.save();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&cfg_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    let composed = compose_query("wolf", &s);
    assert!(composed.contains("wolf"));
    assert!(composed.contains("-rating:e"));
    assert!(composed.contains("score:>=50"));
    assert!(composed.contains("order:score"));
    assert!(composed.contains("-animated"));
    assert!(composed.contains("pool:any"));
    assert!(composed.contains("-gore"));
    assert!(composed.contains("-young"));

    let composed2 = compose_query("wolf pool:123 -animated", &s);
    assert!(!composed2.contains("pool:any"));
    assert_eq!(composed2.matches("-animated").count(), 1);
}

#[test]
fn test_tag_autocomplete() {
    let mut ac = TagAutocompleteState::new();
    let (res, cur) = TagAutocompleteState::insert_tag("solo can", 8, "canine", false);
    assert_eq!(res, "solo canine");
    assert_eq!(cur, 11);

    let (res2, cur2) = TagAutocompleteState::insert_tag("solo -can", 9, "canine", true);
    assert_eq!(res2, "solo -canine ");
    assert_eq!(cur2, 13);

    let matches = ac.local_matches("wo");
    assert!(!matches.is_empty());
    assert_eq!(matches[0].name, "wolf");

    let mut tags = BTreeMap::new();
    tags.insert("species".to_string(), vec!["custom_species_xyz".to_string()]);
    let p = Post {
        id: 1,
        tags,
        ..Default::default()
    };
    ac.learn_post_tags(&p);
    let m2 = ac.local_matches("custom_species");
    assert_eq!(m2.len(), 1);
    assert_eq!(m2[0].category, 5);
}

#[test]
fn test_history_sqlite_and_auto_prune() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("history.db");
    let target_file = dir.path().join("artists").join("solo_artist").join("803.png");
    fs::create_dir_all(target_file.parent().unwrap()).unwrap();
    fs::write(&target_file, b"pngdata").unwrap();

    let post = make_post(803);
    let hist = History::open(&db_path);
    hist.add(&post, &target_file);
    assert!(hist.has(803));
    assert_eq!(
        hist.path_for(803),
        Some(target_file.to_string_lossy().into_owned())
    );
    assert_eq!(hist.len(), 1);

    // Reopen from disk
    let hist2 = History::open(&db_path);
    assert!(hist2.has(803));

    // Delete file -> History detects removal and prunes
    fs::remove_file(&target_file).unwrap();
    assert!(!hist2.has(803));
    assert!(hist2.path_for(803).is_none());
    assert_eq!(hist2.len(), 0);
}

#[test]
fn test_spank_bank_posts_pools_artists() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("spank_bank.db");
    let bank = SpankBank::open(&db_path);

    let p1 = make_post(201);
    let p2 = make_post(202);
    assert!(!bank.has(201));
    bank.add(&p1);
    assert!(bank.has(201));
    assert_eq!(bank.len(), 1);
    assert!(bank.toggle(&p2));
    assert_eq!(bank.len(), 2);
    assert!(!bank.toggle(&p2));
    assert_eq!(bank.len(), 1);
    assert_eq!(bank.get_all()[0].id, 201);

    // Pools
    let pool1 = Pool {
        id: 55,
        name: "Test_Comic_1".to_string(),
        post_ids: vec![101, 102],
        category: "series".to_string(),
        description: "A fun comic".to_string(),
        is_active: true,
    };
    bank.add_pool(&pool1);
    assert!(bank.has_pool(55));
    assert_eq!(bank.pool_count(), 1);
    assert_eq!(bank.get_all_pools()[0].display_name(), "Test Comic 1");

    // Artists
    assert!(bank.toggle_artist("  Some Artist  "));
    assert!(bank.has_artist("some_artist"));
    assert!(bank.has_artist("SOME ARTIST"));
    bank.add_artist("another_artist");
    assert_eq!(bank.artist_count(), 2);

    // Reopen persistence
    let bank2 = SpankBank::open(&db_path);
    assert!(bank2.has(201));
    assert!(bank2.has_pool(55));
    assert!(bank2.has_artist("some_artist"));
    assert_eq!(bank2.artist_count(), 2);

    bank2.clear();
    bank2.clear_pools();
    bank2.clear_artists();
    assert_eq!(bank2.len(), 0);
    assert_eq!(bank2.pool_count(), 0);
    assert_eq!(bank2.artist_count(), 0);
}

fn tiny_png_bytes() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .unwrap();
    buf.into_inner()
}

#[test]
fn test_image_cache_and_fetch_image() {
    let dir = tempfile::tempdir().unwrap();
    let cache = ImageCache::new(&dir.path().join("cache"), 120);

    assert_eq!(
        cache.preview_path(123, "png"),
        dir.path().join("cache").join("previews").join("123.png")
    );
    assert_eq!(
        cache.thumb_path(123, "jpg"),
        dir.path().join("cache").join("thumbs").join("123.jpg")
    );

    let cimg = Arc::new(egui::ColorImage::new([4, 4], egui::Color32::RED));
    cache.set_memory_preview(123, Arc::clone(&cimg));
    assert!(cache.get_memory_preview(123).is_some());
    assert!(cache.get_memory_preview(999).is_none());
    cache.set_memory_thumb(123, cimg);
    assert!(cache.get_memory_thumb(123).is_some());

    let png = tiny_png_bytes();
    let local_file = dir.path().join("downloaded_123.png");
    fs::write(&local_file, &png).unwrap();

    let calls = AtomicUsize::new(0);
    let loaded = fetch_image_with_fetcher("http://fake/img.png", None, Some(&local_file), |_| {
        calls.fetch_add(1, Ordering::Relaxed);
        Ok(vec![])
    })
    .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert_eq!(loaded.width(), 4);

    // Non-image video local_path should fall through to network/cache without error
    let video_file = dir.path().join("video_123.webm");
    fs::write(&video_file, b"not-an-image").unwrap();
    let new_cache = cache.preview_path(456, "png");
    let loaded_net = fetch_image_with_fetcher(
        "http://fake/456.png",
        Some(&new_cache),
        Some(&video_file),
        |_| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(png.clone())
        },
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(loaded_net.width(), 4);
    assert!(new_cache.exists());

    // Subsequent call hits disk cache without calling fetcher
    let loaded_cached = fetch_image_with_fetcher(
        "http://fake/456.png",
        Some(&new_cache),
        None,
        |_| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(vec![])
        },
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(loaded_cached.width(), 4);

    // find_local_image_path checks history & preview cache
    let hist = History::open(&dir.path().join("hist.db"));
    let p777 = make_post(777);
    assert!(cache.find_local_image_path(&p777, Some(&hist), None).is_none());
    let hist_file = dir.path().join("777.png");
    fs::write(&hist_file, &png).unwrap();
    hist.add(&p777, &hist_file);
    assert_eq!(
        cache.find_local_image_path(&p777, Some(&hist), None),
        Some(hist_file)
    );
}

#[test]
fn test_make_download_jobs_and_tags_txt() {
    let mut p_in_pool = make_post(801);
    p_in_pool.pools = vec![88];
    p_in_pool.tags.insert("artist".into(), vec!["comic_artist".into()]);

    let mut p_no_pool = make_post(803);
    p_no_pool.pools.clear();
    p_no_pool.tags.insert("artist".into(), vec!["solo_artist".into()]);

    let bank_pool = Pool {
        id: 88,
        name: "Bank_Comic".to_string(),
        post_ids: vec![801, 802],
        category: "series".to_string(),
        ..Default::default()
    };
    let src = Source::new(SourceKind::SpankBank, "Spank Bank", "spank_bank");

    let jobs_pool = make_download_jobs(
        &[p_in_pool.clone(), p_no_pool.clone()],
        &src,
        "/dl",
        "pool",
        true,
        true,
        std::slice::from_ref(&bank_pool),
    );
    assert_eq!(
        build_target(&jobs_pool[0], |_| None),
        PathBuf::from("/dl/pools/Bank Comic (88)/0001_801.png")
    );
    assert_eq!(
        build_target(&jobs_pool[1], |_| None),
        PathBuf::from("/dl/artists/solo_artist/803.png")
    );

    let jobs_artist = make_download_jobs(
        &[p_in_pool.clone()],
        &src,
        "/dl",
        "artist",
        false,
        true,
        &[bank_pool],
    );
    assert_eq!(
        build_target(&jobs_artist[0], |_| None),
        PathBuf::from("/dl/artists/comic_artist/801.png")
    );

    let txt = format_tags_txt(&p_in_pool);
    assert!(txt.starts_with("comic artist, "));
    assert!(txt.contains("wolf"));
    assert!(txt.ends_with('\n'));
}

#[test]
fn test_tabs_navigation_and_viewer_state() {
    let p1 = make_post(801);
    let p2 = make_post(802);
    let p3 = make_post(803);

    let mut tab = ResultTab::new(1, Source::new(SourceKind::Search, "wolf", "wolf"));
    assert_eq!(tab.title(), "wolf");
    let dl: HashSet<u64> = [801].into_iter().collect();
    tab.add_posts(vec![p1.clone(), p2.clone()], &dl);
    assert_eq!(tab.posts.len(), 2);
    assert!(tab.downloaded_ids.contains(&801));
    tab.select_all();
    assert_eq!(tab.selected_posts().len(), 2);
    tab.clear_selection();
    assert!(tab.selected_posts().is_empty());

    // Tab snapshots for Back/Forward navigation
    let snap = tab.snapshot();
    tab.history_back.push(snap);
    tab.clear_posts();
    tab.source = Source::new(SourceKind::Artist, "someartist", "someartist");
    assert_eq!(tab.title(), "Artist: someartist");
    assert_eq!(tab.history_back.len(), 1);
    assert_eq!(tab.history_back[0].posts.len(), 2);

    // Viewer state + slideshow + edge clicks
    let mut v = ViewerState::new(vec![p1, p2, p3], 0, 3.0, false, false);
    assert!(v.true_fullscreen);
    assert!((v.slideshow_interval - 3.0).abs() < 1e-6);

    v.start_slideshow();
    assert!(v.slideshow_playing);
    assert!(v.tick_slideshow());
    assert_eq!(v.current_post().unwrap().id, 802);
    assert!(v.tick_slideshow());
    assert_eq!(v.current_post().unwrap().id, 803);
    // Loop wraps back to 0
    assert!(v.tick_slideshow());
    assert_eq!(v.current_post().unwrap().id, 801);

    // Speed shortcuts
    assert!((v.increase_speed() - 2.5).abs() < 1e-6);
    assert!((v.decrease_speed() - 3.0).abs() < 1e-6);
    v.set_interval(0.5);
    assert!((v.slideshow_interval - 0.5).abs() < 1e-6);

    // Edge click zones (width 1000 -> edge 250)
    assert_eq!(ViewerState::edge_zone_width(1000.0), 250.0);
    assert_eq!(v.click_edge_at(920.0, 1000.0), Some(true));
    assert_eq!(v.current_post().unwrap().id, 802);
    assert_eq!(v.click_edge_at(500.0, 1000.0), None);
    assert_eq!(v.current_post().unwrap().id, 802);
    assert_eq!(v.click_edge_at(50.0, 1000.0), Some(true));
    assert_eq!(v.current_post().unwrap().id, 801);

    // Shuffle mode picks one of the other posts
    v.slideshow_shuffle = true;
    assert!(v.tick_slideshow());
    let id_after_shuffle = v.current_post().unwrap().id;
    assert!([802, 803].contains(&id_after_shuffle));

    // Toggle slideshow pause/resume
    v.toggle_slideshow();
    assert!(!v.slideshow_playing);
    v.toggle_slideshow();
    assert!(v.slideshow_playing);
}

#[test]
fn test_first_run_splash_state_and_settings_trigger() {
    let dir = tempfile::tempdir().unwrap();
    let cfg_path = dir.path().join("settings.json");

    let mut s = Settings::default();
    assert!(s.needs_first_run_setup());

    // Partial or whitespace-only credentials still require setup.
    s.username = "wolfuser".to_string();
    assert!(s.needs_first_run_setup());
    s.api_key = "   ".to_string();
    assert!(s.needs_first_run_setup());

    // Only API key without username still requires setup.
    s.username.clear();
    s.api_key = "secret_key".to_string();
    assert!(s.needs_first_run_setup());

    // Existing/migrated configs with both credentials skip the first-run splash automatically.
    s.username = "wolfuser".to_string();
    s.api_key = "secret_key".to_string();
    assert!(!s.first_run_complete);
    assert!(!s.needs_first_run_setup());

    // Skipping first-run onboarding marks it complete even without credentials.
    let skipped = Settings {
        first_run_complete: true,
        ..Default::default()
    };
    assert!(!skipped.needs_first_run_setup());

    // Splash state trims values, sets first_run_complete, and persists to disk.
    let mut draft_settings = Settings::load_from(&cfg_path);
    assert!(draft_settings.needs_first_run_setup());
    let mut splash = FirstRunSplashState::from_settings(&draft_settings, false);
    assert!(!splash.has_credentials());
    splash.username = "  foxuser  ".to_string();
    assert!(!splash.has_credentials());
    splash.api_key = "  my_api_key  ".to_string();
    assert!(splash.has_credentials());

    splash.apply_to(&mut draft_settings);
    assert!(draft_settings.first_run_complete);
    assert_eq!(draft_settings.username, "foxuser");
    assert_eq!(draft_settings.api_key, "my_api_key");
    assert!(!draft_settings.needs_first_run_setup());

    draft_settings.save();
    let reloaded = Settings::load_from(&cfg_path);
    assert!(reloaded.first_run_complete);
    assert_eq!(reloaded.username, "foxuser");
    assert_eq!(reloaded.api_key, "my_api_key");
    assert!(!reloaded.needs_first_run_setup());
}

#[test]
fn test_first_run_splash_layout_bounds_fullscreen_and_min_size() {
    let ctx = egui::Context::default();
    theme::configure_style(&ctx);

    let mut splash = FirstRunSplashState::from_settings(&Settings::default(), false);
    for (width, height) in [(1920.0_f32, 1080.0_f32), (1400.0, 900.0), (800.0, 560.0)] {
        let mut card_rect = egui::Rect::NOTHING;
        for _ in 0..2 {
            let raw_input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, height),
                )),
                ..Default::default()
            };
            let _ = ctx.run(raw_input, |ctx| {
                let mut actions = Vec::new();
                card_rect = show_first_run_splash(ctx, &mut splash, None, &mut actions);
            });
        }
        assert!(
            card_rect.min.x >= 0.0 && card_rect.max.x <= width,
            "viewport {width}x{height} card overflowed horizontally: {card_rect:?}"
        );
        assert!(
            (card_rect.center().x - width * 0.5).abs() <= 4.0,
            "viewport {width}x{height} card not horizontally centered: {card_rect:?}"
        );
        assert!(
            card_rect.width() <= 684.0,
            "viewport {width}x{height} card exceeded max width: {card_rect:?}"
        );
        assert!(
            card_rect.height() >= 250.0 && card_rect.height() <= 660.0,
            "viewport {width}x{height} card unexpected height (not stacking vertically or stretched): {card_rect:?}"
        );
        if height >= 900.0 {
            assert!(
                card_rect.max.y <= height,
                "viewport {width}x{height} card overflowed vertically: {card_rect:?}"
            );
        }
    }
}
