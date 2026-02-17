Name:           datalens-mcp
Version:        0.1.0
Release:        1%{?dist}
Summary:        MCP server for Yandex DataLens API

License:        Apache-2.0
URL:            https://github.com/snevolin/datalens-mcp
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust

%description
datalens-mcp is a Model Context Protocol (MCP) server for Yandex DataLens.
It exposes DataLens RPC methods as MCP tools over stdio transport.

%prep
%setup -q

%build
cargo build --release

%install
install -Dm755 target/release/datalens-mcp \
    %{buildroot}%{_bindir}/datalens-mcp
install -Dm644 man/datalens-mcp.1 \
    %{buildroot}%{_mandir}/man1/datalens-mcp.1

%files
%{_bindir}/datalens-mcp
%{_mandir}/man1/datalens-mcp.1*
%license LICENSE
%doc README.md README_ru.md

%changelog
* Tue Feb 17 2026 Stanislav Nevolin <stanislav@nevolin.info> - 0.1.0-1
- Initial package
