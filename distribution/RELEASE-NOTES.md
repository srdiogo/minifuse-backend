MiniFuse for Linux includes the session backend, desktop mixer and optional
Omarchy panel.

- Ubuntu 26.04 LTS x86-64 uses the DEB.
- Fedora 45 x86-64 uses the RPM.
- Arch/Omarchy x86-64 uses the split Arch packages. The panel archive is optional.
- Verify downloads with `sha256sum -c SHA256SUMS --ignore-missing`.

MiniFuse 4 has physical validation. MiniFuse 1, 2 and 2 OTG controls remain
experimental and appear automatically when firmware, driver and readback checks pass. The MF4 A button and the model-independent `minifuse-control --toggle` command share the same desktop behavior. See the [installation guide](https://github.com/srdiogo/minifuse-backend/blob/main/docs/NATIVE_PACKAGES.md)
for kernel headers, DKMS and session startup.
