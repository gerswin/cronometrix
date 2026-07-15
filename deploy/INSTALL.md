# Private installation and upgrade

Cronometrix releases are private, immutable bundles produced by the `Release`
GitHub Actions workflow. Anonymous downloads and `curl | bash` are not
supported.

## Operator handoff

1. Download the private `cronometrix-private-release-*` Actions artifact while
   authenticated to GitHub. It contains one `.tar.gz` and its external
   `.tar.gz.sha256` file.
2. Verify the external checksum before extraction:

   ```bash
   sha256sum -c cronometrix-*.tar.gz.sha256
   ```

3. Inspect the archive before extraction. It must list exactly the five regular
   files below, without absolute paths, `..`, duplicate names, links, or extra
   members:

   ```text
   install.sh
   docker-compose.yml
   release-manifest.env
   nginx.conf
   SHA256SUMS
   ```

4. Transfer the archive and checksum to the client server over an authenticated
   channel such as SSH/SFTP. Extract into an empty root-owned directory, enter
   that directory, then run:

   ```bash
   sudo bash install.sh
   ```

The installer revalidates the exact member set, the internal `SHA256SUMS`, the
strict release manifest, Linux/amd64 compatibility, Docker versions, disk
space, and all image digests before changing the installation.

For unattended operation, export the named `CRONOMETRIX_*` variables requested
by the script. Secrets are never accepted as positional arguments.

## GHCR credential lifecycle

Create one distinct technical account token per installation. Use a classic
PAT with package-read access only; do not grant package write, repository
administration, or unrelated scopes. Record its owner, expiry, client slug,
and revocation ticket in the operator credential register.

The token is entered once through silent input. Docker retains it only in the
root-owned `/opt/cronometrix/.docker/config.json` (`0600`); it is not copied to
the runtime `.env` or release manifest. The data directory and Docker config
remain `0700` and are preserved across reruns.

To rotate a token, obtain the replacement PAT and rerun the same verified
release bundle. Revoke the previous PAT only after the installer completes and
an authenticated image pull succeeds. To revoke access permanently, revoke the
PAT in GitHub and remove `/opt/cronometrix/.docker/config.json` on the server.

Offline restarts continue to work from already pulled digest-pinned images:

```bash
cd /opt/cronometrix
sudo DOCKER_CONFIG=/opt/cronometrix/.docker \
  docker compose --env-file .env --env-file release-manifest.env up -d
```

An upgrade requires the new private bundle and temporary GHCR access to pull
new digests. Mutable tags such as `latest` are never used.

## Transaction and rollback

Before an upgrade, the installer saves the current Compose file, Nginx file,
manifest, runtime environment, container image inventory, and a consistent
SQLite backup under `/opt/cronometrix/releases/rollback/<UTC timestamp>/`.
Candidate files are installed atomically. The installer validates Compose,
pulls all four pinned images, starts the API, then web and gateway, and keeps
Cloudflare stopped until local health, setup, upload-limit, and container
health probes pass.

If any candidate step fails, the installer stops it, restores the prior files
and SQLite database, restarts the previous release, checks gateway health, and
returns a non-zero status. Only the two newest rollback directories are kept,
and pruning occurs only after candidate health succeeds.

For an operator-initiated rollback, choose a timestamp and restore its files
while the stack is stopped:

```bash
cd /opt/cronometrix
sudo DOCKER_CONFIG=/opt/cronometrix/.docker \
  docker compose --env-file .env --env-file release-manifest.env down
sudo cp releases/rollback/TIMESTAMP/docker-compose.yml docker-compose.yml
sudo cp releases/rollback/TIMESTAMP/release-manifest.env release-manifest.env
sudo cp releases/rollback/TIMESTAMP/nginx.conf nginx.conf
sudo cp releases/rollback/TIMESTAMP/cronometrix.db data/cronometrix.db
sudo DOCKER_CONFIG=/opt/cronometrix/.docker \
  docker compose --env-file .env --env-file release-manifest.env up -d
curl -fsS http://127.0.0.1:8080/api/v1/health
```

## Network and secret boundaries

The Cloudflare remotely managed public hostname must target
`http://gateway:8080`. API and web containers remain internal; the local entry
point is `http://127.0.0.1:8080`.

Each installation keeps its license, administrator credential, Cloudflare
tunnel token, and GHCR PAT private. License and admin inputs are sent as JSON
through stdin and are immediately unset; neither is written to `.env`. A
rerun preserves `JWT_SECRET`, `DEVICE_CREDS_KEY`, application data, and Docker
credentials. A different supplied license key is a hard failure.
