Name:           e621-cruiser
Version:        %{?_version}%{!?_version:1.1.0}
Release:        1%{?dist}
Summary:        Desktop browser and downloader for e621

License:        MIT
URL:            https://github.com/morganlowe/E621-cruiser
Source0:        e621-cruiser-%{version}.tar.gz

BuildArch:      noarch
Requires:       python3 >= 3.10
Requires:       python3-pyside6
Requires:       python3-requests

%description
E621 Cruiser is a PySide6 desktop application to browse, filter, favorite,
and download posts, pools, artists, and favorites from e621.

%prep
%autosetup -n e621-cruiser-%{version}

%build
# Pure Python application; nothing to compile.

%install
install -dm755 %{buildroot}%{_datadir}/e621-cruiser/e621cruiser
install -dm755 %{buildroot}%{_datadir}/e621-cruiser/assets
install -m644 e621cruiser/*.py %{buildroot}%{_datadir}/e621-cruiser/e621cruiser/
install -m644 assets/e621-cruiser.png %{buildroot}%{_datadir}/e621-cruiser/assets/
install -m644 assets/e621-cruiser.svg %{buildroot}%{_datadir}/e621-cruiser/assets/

install -Dm755 packaging/e621-cruiser.sh %{buildroot}%{_bindir}/e621-cruiser
ln -sf e621-cruiser %{buildroot}%{_bindir}/e621cruiser

install -Dm644 packaging/e621-cruiser.desktop %{buildroot}%{_datadir}/applications/e621-cruiser.desktop
install -Dm644 assets/e621-cruiser.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/e621-cruiser.svg
install -Dm644 assets/e621-cruiser.png %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/e621-cruiser.png

%files
%license LICENSE
%doc README.md
%{_bindir}/e621-cruiser
%{_bindir}/e621cruiser
%{_datadir}/e621-cruiser/
%{_datadir}/applications/e621-cruiser.desktop
%{_datadir}/icons/hicolor/scalable/apps/e621-cruiser.svg
%{_datadir}/icons/hicolor/256x256/apps/e621-cruiser.png

%changelog
* Thu Nov 19 2026 Morgan Lowe <https://github.com/morganlowe/E621-cruiser> - 1.1.0-1
- Initial RPM package for Fedora
