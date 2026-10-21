window.BENCHMARK_DATA = {
  "lastUpdate": 1792569868937,
  "repoUrl": "https://github.com/LordPhenixDeNetra/Ndaal",
  "entries": {
    "micro": [
      {
        "commit": {
          "author": {
            "name": "LordPhenixDeNetra",
            "username": "LordPhenixDeNetra",
            "email": "69590712+LordPhenixDeNetra@users.noreply.github.com"
          },
          "committer": {
            "name": "MOUSSA THIOR",
            "username": "LordPhenixDeNetra",
            "email": "69590712+LordPhenixDeNetra@users.noreply.github.com"
          },
          "id": "3c483a2b2844bd1d612323c0661142874cc9e707",
          "message": "fix(ci): return to the tested commit after creating bench-data\n\ngit switch - needs a previous branch in the reflog, which the Actions checkout does not keep: the first bench run created and pushed bench-data, then failed with invalid reference @{-1}. Check out GITHUB_SHA instead.\n\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-10-14T01:05:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/3c483a2b2844bd1d612323c0661142874cc9e707"
        },
        "date": 1791940244071,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5445,
            "range": "± 214",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 6005,
            "range": "± 194",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 190354,
            "range": "± 4884",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "dependabot[bot]",
            "username": "dependabot[bot]",
            "email": "49699333+dependabot[bot]@users.noreply.github.com"
          },
          "committer": {
            "name": "MOUSSA THIOR",
            "username": "LordPhenixDeNetra",
            "email": "69590712+LordPhenixDeNetra@users.noreply.github.com"
          },
          "id": "59effb00d852d7c0ab1a65838d0f7403ca2ef8a5",
          "message": "build(deps): Bump tokio from 1.53.2 to 1.53.3\n\nBumps [tokio](https://github.com/tokio-rs/tokio) from 1.53.2 to 1.53.3.\n- [Release notes](https://github.com/tokio-rs/tokio/releases)\n- [Commits](https://github.com/tokio-rs/tokio/compare/tokio-1.53.2...tokio-1.53.3)\n\n---\nupdated-dependencies:\n- dependency-name: tokio\n  dependency-version: 1.53.3\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-10-20T21:06:02Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/59effb00d852d7c0ab1a65838d0f7403ca2ef8a5"
        },
        "date": 1792569868237,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5037,
            "range": "± 285",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 5751,
            "range": "± 162",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 167164,
            "range": "± 11121",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}