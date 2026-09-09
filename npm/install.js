#!/usr/bin/env node
'use strict';
// postinstall: put this platform's prebuilt binaries in vendor/, and refuse
// rather than install anything this package cannot verify.
//
// Two checks, in this order, both fail-closed:
//
//   1. SHA-256 against the digest pinned in package.json by the release that
//      built the asset. The digest travels inside the npm tarball, so it is
//      not served by the host serving the bytes it vouches for. A wrapper
//      packed by hand carries no pinned digest; that case falls back to the
//      release's own SHA256SUMS and SAYS SO, because it is a weaker claim.
//   2. The build provenance the release attested (SLSA, via Sigstore), when
//      `gh` is on PATH: it ties the asset to the workflow, commit and runner
//      that produced it. A FAILED verification always aborts. An ABSENT
//      verifier prints the command and continues on the default
//      `<PREFIX>_ATTESTATION=check`, and aborts on `=require`; `=skip` does
//      not run it at all and says so loudly. The dial only moves this layer —
//      the digest check above is fatal in every mode.
//
// Nothing here ever runs, chmods, or copies a byte that failed a check.
var fs = require('fs');
var os = require('os');
var path = require('path');
var https = require('https');
var crypto = require('crypto');
var child = require('child_process');
var execFileSync = child.execFileSync;
var resolve = require('./lib/resolve');
var manifest = require('./package.json');

var vendor = path.join(__dirname, 'vendor');

function fail(message) {
  console.error(manifest.name + ': ' + message);
  process.exit(1);
}

function binaries() {
  return Object.keys(manifest.x0k.commands).map(function (c) {
    return manifest.x0k.commands[c];
  });
}

// GitHub redirects release downloads to its object store, so redirects are the
// normal path here, not an edge case.
function get(url, redirects) {
  return new Promise(function (ok, no) {
    https.get(url, { headers: { 'user-agent': manifest.name } }, function (res) {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        res.resume();
        if ((redirects || 0) > 5) { no(new Error('too many redirects for ' + url)); return; }
        ok(get(new URL(res.headers.location, url).toString(), (redirects || 0) + 1));
        return;
      }
      if (res.statusCode !== 200) {
        res.resume();
        no(new Error('HTTP ' + res.statusCode + ' for ' + url));
        return;
      }
      var chunks = [];
      res.on('data', function (c) { chunks.push(c); });
      res.on('end', function () { ok(Buffer.concat(chunks)); });
    }).on('error', no);
  });
}

function expectedSum(sums, asset) {
  var lines = sums.split('\n');
  for (var i = 0; i < lines.length; i += 1) {
    var parts = lines[i].trim().split(/\s+/);
    if (parts.length >= 2 && parts[parts.length - 1].replace(/^\*/, '') === asset) {
      return parts[0];
    }
  }
  throw new Error(asset + ' is not listed in ' + manifest.x0k.checksums);
}

// The build provenance, checked where a verifier exists. `gh` shells out to a
// public transparency log, so this is the layer that says WHO built the bytes,
// not merely that they are the bytes somebody listed. A failure here is fatal:
// an asset that fails attestation is exactly the case this check is for.
function attest(archive) {
  var mode = resolve.attestationMode(manifest, process.env);
  var args = resolve.attestationCommand(manifest, archive);
  var line = 'gh ' + args.join(' ');
  if (mode === 'skip') {
    console.error(
      manifest.name + ': WARNING — build provenance NOT checked (' +
      resolve.attestationVar(manifest) + '=skip). The digest matched, so these are the ' +
      'bytes this package expected; nothing has attested WHO built them. To check: ' + line
    );
    return;
  }
  try {
    child.execFileSync('gh', args, { stdio: 'inherit' });
  } catch (e) {
    // ENOENT is "no verifier on this machine"; any other non-zero exit is the
    // verifier itself saying no, and that is never survivable.
    if (e && e.code === 'ENOENT') {
      if (mode === 'require') {
        fail(
          resolve.attestationVar(manifest) + '=require, but the GitHub CLI is not installed, ' +
          'so the build provenance cannot be checked. Install it (https://cli.github.com), ' +
          'or set ' + resolve.attestationVar(manifest) + '=check to accept a digest-only install.'
        );
      }
      console.error(
        manifest.name + ': note — build provenance not checked (no `gh` on PATH). ' +
        'The digest matched. To check the provenance too: ' + line
      );
      return;
    }
    fail(
      'the build provenance for ' + path.basename(archive) + ' did not verify. Refusing to ' +
      'install; nothing was written. This is what a tampered or re-uploaded release asset ' +
      'looks like — and also what an unauthenticated or rate-limited `gh` looks like, so if ' +
      'you believe the release is good, run `' + line + '` yourself before setting ' +
      resolve.attestationVar(manifest) + '=skip.'
    );
  }
}

function install(entry, dir) {
  binaries().forEach(function (bin) {
    var source = path.join(dir, bin + entry.exe);
    if (!fs.existsSync(source)) { fail(source + ' does not exist'); }
    var target = path.join(vendor, bin + entry.exe);
    fs.copyFileSync(source, target);
    fs.chmodSync(target, 0o755);
  });
}

function main() {
  var entry = resolve.resolveTarget(manifest, process.platform, process.arch);
  // Read the dial before anything is fetched: a misspelt value should refuse
  // at once, not after a download whose result it would have governed.
  resolve.attestationMode(manifest, process.env);
  fs.mkdirSync(vendor, { recursive: true });
  // The local rung: a directory of binaries this machine already has. A nix
  // build, a distro package, or a maintainer testing an unreleased build sets
  // this, and the install never reaches the network.
  var provided = process.env[resolve.binaryDirVar(manifest)];
  if (provided) { install(entry, provided); return Promise.resolve(); }
  var asset = resolve.assetName(manifest, entry);
  var pinned = resolve.pinnedDigest(manifest, entry);
  var wanted = pinned
    ? Promise.resolve(pinned)
    : get(resolve.checksumUrl(manifest, process.env)).then(function (sums) {
        console.error(
          manifest.name + ': note — this copy of the wrapper carries no pinned digest, ' +
          'so ' + asset + ' is being checked against the release\'s own ' +
          manifest.x0k.checksums + '. A wrapper published by the release workflow ' +
          'pins the digest instead.'
        );
        return expectedSum(sums.toString('utf8'), asset);
      });
  return wanted.then(function (want) {
    return get(resolve.assetUrl(manifest, entry, process.env)).then(function (bytes) {
      var got = crypto.createHash('sha256').update(bytes).digest('hex');
      if (got !== want) {
        fail(
          'digest mismatch for ' + asset + ': expected ' + want + ', got ' + got +
          '. Refusing to install — nothing was written.'
        );
      }
      var tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'prebuilt-'));
      var archive = path.join(tmp, asset);
      fs.writeFileSync(archive, bytes);
      attest(archive);
      // `tar` reads both shapes we ship: GNU tar for the .tar.gz on Linux, and
      // bsdtar — which is what `tar` is on macOS and on Windows 10 and later —
      // for the .zip. So there is no unzip dependency and no bundled unpacker.
      execFileSync('tar', ['-xf', archive, '-C', tmp], { stdio: 'inherit' });
      fs.unlinkSync(archive);
      install(entry, tmp);
      fs.rmSync(tmp, { recursive: true, force: true });
    });
  });
}

// Through a promise, so that a refusal thrown synchronously — an unsupported
// platform, a misspelt dial — reaches `fail` and prints the sentence written
// for it, rather than a stack trace at somebody's `npm install`.
Promise.resolve().then(main).catch(function (e) { fail(e.message); });
