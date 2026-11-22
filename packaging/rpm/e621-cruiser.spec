%global debug_package %{nil}

Name:           e621-cruiser
Version:        %{?_version}%{!?_version:2.0.0}
Release:        1%{?dist}
Summary:        Native Rust desktop browser and downloader for e621

License:        MIT
URL:            https://github.com/morganlowe/E621-cruiser
Source0:        e621-cruiser-%{version}.tar.gz

BuildRequires:  cargo rust gcc

%description
E621 Cruiser is a fast, self-contained Rust/egui desktop application to browse,
filter, favorite, and download posts, pools, artists, and favorites from e621.

%prep
%autosetup -n e621-cruiser-%{version}

%build
cargo build --release --locked

%install
install -Dm755 target/release/e621-cruiser %{buildroot}%{_bindir}/e621-cruiser
ln -sf e621-cruiser %{buildroot}%{_bindir}/e621cruiser

install -Dm644 packaging/e621-cruiser.desktop %{buildroot}%{_datadir}/applications/e621-cruiser.desktop
install -Dm644 assets/e621-cruiser.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/e621-cruiser.svg
install -Dm644 assets/e621-cruiser.png %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/e621-cruiser.png

%files
%license LICENSE
%doc README.md
%{_bindir}/e621-cruiser
%{_bindir}/e621cruiser
%{_datadir}/applications/e621-cruiser.desktop
%{_datadir}/icons/hicolor/scalable/apps/e621-cruiser.svg
%{_datadir}/icons/hicolor/256x256/apps/e621-cruiser.png

%changelog
* Sun Nov 22 2026 Morgan Lowe <https://github.com/morganlowe/E621-cruiser> - 2.0.0-1
- Rewrite in Rust with egui/eframe as a single self-contained native binary
- Add first-run API key onboarding splash screen and setup guide
