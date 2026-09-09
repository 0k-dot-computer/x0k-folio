#!/usr/bin/env node
'use strict';
// Launcher for `x0k-folio`. npm links this file; it runs the real binary that
// install.js put in vendor/, and exits with its status.
var fs = require('fs');
var path = require('path');
var spawnSync = require('child_process').spawnSync;
var resolve = require('../lib/resolve');
var manifest = require('../package.json');

var entry = resolve.resolveTarget(manifest, process.platform, process.arch);
var binary = path.join(__dirname, '..', resolve.binaryFile(manifest, entry, 'x0k-folio'));
if (!fs.existsSync(binary)) {
  console.error(manifest.name + ': ' + binary + ' is missing — the install step did not run.');
  console.error('Reinstall without --ignore-scripts, or run: node ' +
    path.join(__dirname, '..', 'install.js'));
  process.exit(1);
}
var run = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
if (run.error) {
  console.error(manifest.name + ': ' + run.error.message);
  process.exit(1);
}
process.exit(run.status === null ? 1 : run.status);
