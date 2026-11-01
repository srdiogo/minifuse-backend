# Project names and compatibility

**MiniFuse for Linux** is the community project family. Its components stay
separate so the application can be used without Omarchy and the panel can be
used without the full application.

| Public name | Repository | Responsibility |
|---|---|---|
| MiniFuse Control | `srdiogo/minifuse-control` | Standalone application |
| MiniFuse Backend | `srdiogo/minifuse-backend` | Shared core, session service, driver and installer |
| MiniFuse for Omarchy | `srdiogo/omarchy-minifuse` | Optional bar widget and panel |

The backend repository was renamed from `srdiogo/mf-service`. GitHub redirects
existing repository links and Git operations. Existing checkouts can update
their remote from inside the backend repository:

```sh
git remote set-url origin https://github.com/srdiogo/minifuse-backend.git
```

Do not create a different repository at the former URL; that would replace its
redirect. See [GitHub's repository rename documentation](https://docs.github.com/en/repositories/creating-and-managing-repositories/renaming-a-repository).

## Installed names stay compatible

This presentation change does not rename installed components or migrate data:

| Identifier | Preserved name |
|---|---|
| Backend package and executable | `mf-service` |
| User unit | `mf-service.service` |
| Shared Rust crate | `mf-core` |
| Driver package and module | `minifuse-driver-dkms`, `minifuse_mod` |
| Service endpoint | `$XDG_RUNTIME_DIR/mf-service/control.sock` |
| Service state | `$XDG_STATE_HOME/mf-service` |
| Historical core backups | `$XDG_STATE_HOME/mf-cli` |
| Presets | `$XDG_DATA_HOME/minifuse/presets` |
| Application identity | `io.github.srdiogo.MiniFuseControl` |
| Plugin identity | `srdiogo.minifuse` |

API v1 and snapshot schema 1 are unchanged. The `source_project: mf-service`
field in historical client fixture locks identifies the original producer;
it is retained with its revision and SHA-256. Previously delivered archives,
package versions, build records and source locks remain unchanged. New source
kits record the canonical backend repository URL.

The project is independent of Arturia. Existing author credits, research
attribution and component licenses remain in place.
