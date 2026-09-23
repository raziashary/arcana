Name:           arcana
Version:        %{_version}
Release:        1%{?dist}
Summary:        AI-powered development assistant with multi-agent workflows

License:        Proprietary
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo >= 1.75
BuildRequires:  nodejs >= 18
BuildRequires:  npm
BuildRequires:  webkit2gtk4.1-devel
BuildRequires:  gtk3-devel
BuildRequires:  openssl-devel

Requires:       webkit2gtk4.1 >= 2.40.0
Requires:       gtk3 >= 3.24.0
Requires:       openssl
Requires:       ca-certificates
Recommends:     git

%description
Arcana is a local-first AI development assistant that orchestrates a fleet
of specialized AI agents to help with coding, planning, code review, and
project management.

Key features:
- Multi-agent architecture: PA, Supervisor, Workers, QC
- Native LLM provider support (Anthropic, OpenAI, Gemini)
- Integrated IDE with git, file browser, and symbol tree
- Recipe-based automation for repeatable workflows
- MCP (Model Context Protocol) server and client
- Graph-based codebase analysis
- Auto-updater with signature verification

%prep
%autosetup

%build
npm ci --prefer-offline
npm run tauri -- build --bundles deb

%install
rm -rf %{buildroot}

# Tauri builds a .deb, extract it for rpm packaging
DEB=$(find src-tauri/target/release/bundle/deb -name "*.deb" | tail -1)
ar x "$DEB" data.tar.xz
tar -xJf data.tar.xz -C %{buildroot}

%files
%license LICENSE
%doc README.md
/usr/bin/arcana
/usr/share/applications/arcana.desktop
/usr/share/icons/hicolor/*/apps/arcana.png
/usr/share/mime/packages/arcana.xml

%post
# Update desktop + icon + MIME databases
update-desktop-database /usr/share/applications 2>/dev/null || :
gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || :
update-mime-database /usr/share/mime 2>/dev/null || :

%preun
# Stop Arcana if running
pkill -x arcana 2>/dev/null || :

%postun
update-desktop-database /usr/share/applications 2>/dev/null || :
gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || :
update-mime-database /usr/share/mime 2>/dev/null || :

%changelog
* %(date "+%a %b %d %Y") Arcana - %{_version}-1
- Automated build
