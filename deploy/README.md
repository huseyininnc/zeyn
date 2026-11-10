# Deploy

Images and configuration of the public sites: `Dockerfile` (targets `docs`, `showcase`,
`components`), `landing/` (zeynsoft.com), `docker-compose.yml` (every container, bound to
`127.0.0.1`) and `nginx/` (the host's nginx in front of them, certificates from certbot).

## Versioned docs

Every released version of the documentation runs as its own container of the `docs` image,
built from that release. The latest release is served at `https://docs.zeynsoft.com/`, and every
release is also served under `/v<version>/`, such as `/v0.0.9/guides/accounts`. The header of
each page has a version picker that opens the same page in another version, or that version's
start page when it has no such page.

The containers read these variables at startup, so an old version learns about new ones from its
configuration, without a rebuild:

| Variable | Meaning | Default |
|---|---|---|
| `ZEYN_DOCS_VERSION` | the version of this build | the crate version |
| `ZEYN_DOCS_VERSIONS` | every version of the picker, newest first, separated by commas | this version only |
| `ZEYN_DOCS_LATEST` | the latest version, served at `/` | the first of `ZEYN_DOCS_VERSIONS` |
| `ZEYN_DOCS_BASE_PATH` | `/v<version>` for a container under a version path, empty at `/` | empty |

Versions are numbers separated by dots. Entries that are not, repeated entries and entries past
the 50th are dropped; a base path other than `/v<version>` is ignored. An older version shows a
note that links to the latest. A container under a version path sends `noindex` and a canonical
URL at `/`, and its sitemap is empty, so search engines index only the latest at `/`.

### Releasing version X.Y.Z

1. Build and push the image from the release tag (or the `docs-X.Y.Z` branch when the docs of a
   release got fixes later):

   ```
   git checkout vX.Y.Z
   docker build -f deploy/Dockerfile --target docs -t docker.zeynsoft.com/zeyn/docs:X.Y.Z .
   docker push docker.zeynsoft.com/zeyn/docs:X.Y.Z
   ```

2. In `docker-compose.yml`, put `X.Y.Z` first in `ZEYN_DOCS_VERSIONS` and set `ZEYN_DOCS_LATEST`
   to it (both under `x-docs-versions`). Point the `docs` service at `docs:X.Y.Z` and set its
   `ZEYN_DOCS_VERSION: "X.Y.Z"`.

3. Add a service for the new version, with the next free port (`v0.0.9` has `3101`, the next
   version takes `3102`):

   ```yaml
     docs-vX.Y.Z:
       image: docker.zeynsoft.com/zeyn/docs:X.Y.Z
       container_name: zeyn-docs-vX.Y.Z
       restart: unless-stopped
       environment:
         <<: *docs-versions
         ZEYN_DOCS_VERSION: "X.Y.Z"
         ZEYN_DOCS_BASE_PATH: /vX.Y.Z
       ports:
         - "127.0.0.1:3102:3008"
       mem_limit: 128m
       cpus: 1.0
       read_only: true
   ```

4. In `nginx/zeyn.zeynsoft.com.conf`, add a block for it inside the `docs.zeynsoft.com` server,
   next to the others (dots escaped):

   ```nginx
       location ~ ^/vX\.Y\.Z(/|$) {
           proxy_pass http://127.0.0.1:3102;
           proxy_set_header Host $host;
           proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
           proxy_set_header X-Forwarded-Proto $scheme;
           proxy_read_timeout 60s;
       }
   ```

5. On the server: copy both files, then

   ```
   docker compose pull
   docker compose up -d
   sudo nginx -t && sudo systemctl reload nginx
   ```

   `up -d` recreates every docs container, since their variables changed; each takes well under
   a second to start. Check `https://docs.zeynsoft.com/vX.Y.Z/` and the picker on a page of an
   older version.

To try two versions locally, run one build at the root and one under a path behind a proxy that
sends `/v0.0.9/` to the second:

```
ZEYN_DOCS_SERVER__ADDR=127.0.0.1:3108 ZEYN_DOCS_VERSION=0.0.10 ZEYN_DOCS_VERSIONS=0.0.10,0.0.9 cargo run -p zeyn-example-docs
ZEYN_DOCS_SERVER__ADDR=127.0.0.1:3101 ZEYN_DOCS_VERSION=0.0.9 ZEYN_DOCS_VERSIONS=0.0.10,0.0.9 ZEYN_DOCS_BASE_PATH=/v0.0.9 cargo run -p zeyn-example-docs
```

In Git Bash on Windows, set `MSYS_NO_PATHCONV=1` and `MSYS2_ENV_CONV_EXCL='*'` first, or the
shell turns `/v0.0.9` into a Windows path and the build falls back to `/`.
