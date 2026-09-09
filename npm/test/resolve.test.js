'use strict';
// Offline pins for the resolution logic: every platform this package ships for
// maps to exactly one asset URL, an unshipped platform refuses with a message
// naming what IS shipped, and a mirror override moves the base and nothing
// else. No network, no install — `tools/ci-npm` runs this anywhere node is.
var assert = require('assert');
var resolve = require('../lib/resolve');
var manifest = require('../package.json');

var keys = Object.keys(manifest.x0k.targets);
assert.ok(keys.length > 0, 'the package declares at least one platform');

keys.forEach(function (key) {
  var split = key.indexOf('-');
  var platform = key.slice(0, split);
  var arch = key.slice(split + 1);
  var entry = resolve.resolveTarget(manifest, platform, arch);
  assert.strictEqual(entry.target, manifest.x0k.targets[key].target, key);

  var url = resolve.assetUrl(manifest, entry, {});
  assert.ok(url.indexOf(manifest.x0k.releaseBase + '/') === 0, url);
  assert.ok(url.indexOf('/' + resolve.releaseTag(manifest) + '/') > 0, url);
  assert.ok(url.lastIndexOf(resolve.assetName(manifest, entry)) ===
    url.length - resolve.assetName(manifest, entry).length, url);

  var override = {};
  override[resolve.mirrorVar(manifest)] = 'https://mirror.example/dl';
  assert.strictEqual(
    resolve.assetUrl(manifest, entry, override),
    url.replace(manifest.x0k.releaseBase, 'https://mirror.example/dl'),
    'the mirror moves the base and nothing else'
  );

  Object.keys(manifest.x0k.commands).forEach(function (command) {
    var file = resolve.binaryFile(manifest, entry, command);
    var expected = 'vendor/' + manifest.x0k.commands[command] + entry.exe;
    assert.strictEqual(file, expected, command + ' on ' + key);
  });
});

assert.throws(
  function () { resolve.resolveTarget(manifest, 'sunos', 'sparc'); },
  function (e) {
    return e.code === 'EUNSUPPORTEDPLATFORM' &&
      e.message.indexOf(manifest.homepage) > 0 &&
      e.message.indexOf(resolve.binaryDirVar(manifest)) > 0;
  },
  'an unshipped platform refuses and says how to proceed without one'
);

// Every command npm links is a command the launchers and the postinstall know.
Object.keys(manifest.bin).forEach(function (command) {
  assert.ok(manifest.x0k.commands[command], command + ' links to no binary');
  assert.strictEqual(manifest.bin[command], 'bin/' + command + '.js', command);
});

// The digests, if this copy carries any: a pinned digest must be a SHA-256 for
// an asset this package actually ships, and every platform must have one — a
// half-pinned wrapper installs verified on one machine and unverified on the
// next, which is worse than either.
var pinned = Object.keys(manifest.x0k.digests || {});
if (pinned.length > 0) {
  var assets = keys.map(function (key) {
    var entry = manifest.x0k.targets[key];
    return manifest.x0k.assetPrefix + '-' + entry.target + entry.archive;
  });
  pinned.forEach(function (asset) {
    assert.ok(assets.indexOf(asset) >= 0, asset + ' is not an asset this package ships');
    assert.ok(/^[0-9a-f]{64}$/.test(manifest.x0k.digests[asset]), asset + ' has no sha256');
  });
  assert.strictEqual(pinned.length, assets.length, 'every platform is pinned or none is');
  keys.forEach(function (key) {
    var split = key.indexOf('-');
    var entry = resolve.resolveTarget(manifest, key.slice(0, split), key.slice(split + 1));
    assert.ok(resolve.pinnedDigest(manifest, entry), key + ' resolves to its pinned digest');
  });
}

// The attestation command names this repository, whoever asks for it.
assert.deepStrictEqual(
  resolve.attestationCommand(manifest, '/tmp/asset.tar.gz'),
  ['attestation', 'verify', '/tmp/asset.tar.gz', '--repo', manifest.x0k.repo]
);
assert.ok(/^[^/]+\/[^/]+$/.test(manifest.x0k.repo), 'repo is <owner>/<name>');

// The dial: `check` by default, three values, and a typo is a refusal rather
// than a silently weaker install.
var v = resolve.attestationVar(manifest);
assert.strictEqual(resolve.attestationMode(manifest, {}), 'check');
['check', 'require', 'skip'].forEach(function (mode) {
  var env = {};
  env[v] = mode;
  assert.strictEqual(resolve.attestationMode(manifest, env), mode);
});
var bogus = {};
bogus[v] = 'yes';
assert.throws(function () { resolve.attestationMode(manifest, bogus); }, /check, require or skip/);

console.log('ok — ' + keys.length + ' platforms, ' +
  Object.keys(manifest.bin).length + ' commands');
