import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compareVersions, newestRelease, versionParts } from '../src/release-version.ts';
test('stable semantic versions compare numerically', () => {
  assert.equal(compareVersions('0.10.0', '0.9.9'), 1);
  assert.equal(compareVersions('v1.0.0', '1.0.0'), 0);
  for (const v of ['1.0', '01.2.3', '1.0.0-beta', '1.0.0+meta', '999999999999999999.0.0']) assert.equal(versionParts(v), null);
});
test('newest stable release ignores dates, drafts and prereleases', () => {
  const release = newestRelease([
    { tag_name: 'v0.4.0', body: 'Notes', html_url: 'https://untrusted.example' },
    { tag_name: 'v2.0.0', draft: true }, { tag_name: 'v3.0.0', prerelease: true },
    { tag_name: 'v1.0.0-beta.1' }, { tag_name: 'v0.3.1' }, null,
  ], '0.3.0');
  assert.equal(release.version, '0.4.0');
  assert.equal(release.notes, 'Notes');
  assert.equal(release.url, 'https://github.com/TimoLehnertz/ungoggled/releases/tag/v0.4.0');
  assert.equal(newestRelease([{ tag_name: 'v0.3.0' }], '0.3.0'), null);
  assert.equal(newestRelease({ error: 'rate limited' }, '0.3.0'), null);
});
