'use strict';
// Which release asset this machine needs, and where it lives.
//
// Pure: no network, no filesystem, no child process — everything here is a
// function of package.json and a (platform, arch) pair. That is what lets
// test/resolve.test.js check every platform this package ships for, on one
// machine, offline. install.js is the only file that reaches the network.

function platformKey(platform, arch) {
  return platform + '-' + arch;
}

// The target this machine installs, or a refusal naming what IS shipped. A
// missing platform is the ordinary case for a new architecture, so the message
// has to be actionable rather than merely negative.
function resolveTarget(manifest, platform, arch) {
  var key = platformKey(platform, arch);
  var entry = manifest.x0k.targets[key];
  if (!entry) {
    var shipped = Object.keys(manifest.x0k.targets).sort().join(', ');
    var error = new Error(
      'no prebuilt binary for ' + key + '. Prebuilt platforms: ' +
      shipped + '. Build from source instead (see ' + manifest.homepage +
      ') and point ' + binaryDirVar(manifest) + ' at the directory holding the binaries.'
    );
    error.code = 'EUNSUPPORTEDPLATFORM';
    throw error;
  }
  return { key: key, target: entry.target, archive: entry.archive, exe: entry.exe };
}

function releaseTag(manifest) {
  return manifest.x0k.tagPrefix + manifest.version;
}

function assetName(manifest, entry) {
  return manifest.x0k.assetPrefix + '-' + entry.target + entry.archive;
}

// Overridable so a consumer behind a firewall, or a build that must not reach
// the forge, can serve the same assets from their own mirror.
function releaseBase(manifest, env) {
  return (env && env[mirrorVar(manifest)]) || manifest.x0k.releaseBase;
}

function assetUrl(manifest, entry, env) {
  return releaseBase(manifest, env) + '/' + encodeURIComponent(releaseTag(manifest)) +
    '/' + assetName(manifest, entry);
}

function checksumUrl(manifest, env) {
  return releaseBase(manifest, env) + '/' + encodeURIComponent(releaseTag(manifest)) +
    '/' + manifest.x0k.checksums;
}

function mirrorVar(manifest) { return manifest.x0k.envPrefix + '_BINARY_MIRROR'; }
function binaryDirVar(manifest) { return manifest.x0k.envPrefix + '_BINARY_DIR'; }
function attestationVar(manifest) { return manifest.x0k.envPrefix + '_ATTESTATION'; }

// How hard the postinstall leans on the build attestation. `check` (the
// default) runs the verifier when there is one and aborts if it says no;
// `require` additionally aborts when there is no verifier; `skip` does not run
// it at all, for a machine that cannot reach the transparency log. In every
// mode the digest check is fatal — this dial only moves the second layer.
function attestationMode(manifest, env) {
  var mode = (env && env[attestationVar(manifest)]) || 'check';
  if (mode !== 'check' && mode !== 'require' && mode !== 'skip') {
    throw new Error(attestationVar(manifest) + ' must be check, require or skip (got ' + mode + ')');
  }
  return mode;
}

// The digest the release workflow pinned into this package for this asset, or
// null when the wrapper was packed by hand. Null is not "unverified" — it is
// "verified against the release's own SHA256SUMS instead", which install.js
// says out loud, because it is the weaker of the two claims.
function pinnedDigest(manifest, entry) {
  var digests = manifest.x0k.digests || {};
  var digest = digests[assetName(manifest, entry)];
  return typeof digest === 'string' && digest.length === 64 ? digest : null;
}

// `gh attestation verify <file> --repo <this>` — the command that checks an
// asset against the build provenance the release workflow attested.
function attestationCommand(manifest, file) {
  return ['attestation', 'verify', file, '--repo', manifest.x0k.repo];
}

// Where install.js puts a binary and where bin/<command>.js looks for it. The
// two sides agree because they are the same function; `vendor/` is written at
// install time and is not in the published tarball.
function binaryFile(manifest, entry, command) {
  var bin = manifest.x0k.commands[command];
  if (!bin) { throw new Error(manifest.name + ' has no command named ' + command); }
  return 'vendor/' + bin + entry.exe;
}

module.exports = {
  platformKey: platformKey,
  resolveTarget: resolveTarget,
  releaseTag: releaseTag,
  assetName: assetName,
  releaseBase: releaseBase,
  assetUrl: assetUrl,
  checksumUrl: checksumUrl,
  mirrorVar: mirrorVar,
  binaryDirVar: binaryDirVar,
  attestationVar: attestationVar,
  attestationMode: attestationMode,
  pinnedDigest: pinnedDigest,
  attestationCommand: attestationCommand,
  binaryFile: binaryFile
};
