window.BENCHMARK_DATA = {
  "lastUpdate": 1793959334573,
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
      },
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
          "id": "392a3bfa0a1af4e01eb4169eaee51ecb41211e9e",
          "message": "docs: check off T02.01 and T02.02\n\nOne pinned thread per core starts and stops cleanly on Linux, macOS and Windows, and the core layout follows the table of ARCHITECTURE 4.1 from 1 to 256 cores; CI green on the three systems. T02.03 waits for the round-trip measurement on the reference machine (D-05, P-53).\n\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-10-25T21:52:58Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/392a3bfa0a1af4e01eb4169eaee51ecb41211e9e"
        },
        "date": 1793002411164,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 3640,
            "range": "± 58",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 3842,
            "range": "± 100",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 172420,
            "range": "± 5401",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1240,
            "range": "± 14",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "392a3bfa0a1af4e01eb4169eaee51ecb41211e9e",
          "message": "docs: check off T02.01 and T02.02\n\nOne pinned thread per core starts and stops cleanly on Linux, macOS and Windows, and the core layout follows the table of ARCHITECTURE 4.1 from 1 to 256 cores; CI green on the three systems. T02.03 waits for the round-trip measurement on the reference machine (D-05, P-53).\n\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-10-25T21:52:58Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/392a3bfa0a1af4e01eb4169eaee51ecb41211e9e"
        },
        "date": 1793089655507,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 7490,
            "range": "± 161",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 7592,
            "range": "± 429",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 205279,
            "range": "± 3336",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1919,
            "range": "± 1",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "213a3a13fa96b5b0fc23e2181664a38483de6b6d",
          "message": "docs: check off T02.05 and T02.06\n\nCI green on pull request 22, including the disk contract on XFS.\n\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-10-28T04:07:34Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/213a3a13fa96b5b0fc23e2181664a38483de6b6d"
        },
        "date": 1793178329531,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 3619,
            "range": "± 66",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 4051,
            "range": "± 210",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 178696,
            "range": "± 4991",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1280,
            "range": "± 10",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "213a3a13fa96b5b0fc23e2181664a38483de6b6d",
          "message": "docs: check off T02.05 and T02.06\n\nCI green on pull request 22, including the disk contract on XFS.\n\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-10-28T04:07:34Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/213a3a13fa96b5b0fc23e2181664a38483de6b6d"
        },
        "date": 1793264598103,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 4969,
            "range": "± 108",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 5638,
            "range": "± 28",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 170552,
            "range": "± 421",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1752,
            "range": "± 2",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "213a3a13fa96b5b0fc23e2181664a38483de6b6d",
          "message": "docs: check off T02.05 and T02.06\n\nCI green on pull request 22, including the disk contract on XFS.\n\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-10-28T04:07:34Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/213a3a13fa96b5b0fc23e2181664a38483de6b6d"
        },
        "date": 1793351496715,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 6287,
            "range": "± 52",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 6992,
            "range": "± 37",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 203787,
            "range": "± 1485",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1696,
            "range": "± 11",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "213a3a13fa96b5b0fc23e2181664a38483de6b6d",
          "message": "docs: check off T02.05 and T02.06\n\nCI green on pull request 22, including the disk contract on XFS.\n\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-10-28T04:07:34Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/213a3a13fa96b5b0fc23e2181664a38483de6b6d"
        },
        "date": 1793437283470,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 6404,
            "range": "± 141",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 7283,
            "range": "± 25",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 215275,
            "range": "± 220",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 2253,
            "range": "± 2",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "5faedc7cd15e695d6abcdf1f14664ccdb8be02fc",
          "message": "docs: ADR-033 bounded torn zone of the log, J02-C1 spec and plan\n\n1 MiB fragments, the end-of-log rule and the writer invariant in FORMATS 2.3, the T02.08 criterion completed with the owner's approval, deferred points P-59 to P-61. ADR-032 added to the index.\n\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-11-01T02:54:34Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/5faedc7cd15e695d6abcdf1f14664ccdb8be02fc"
        },
        "date": 1793526743647,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 7675,
            "range": "± 180",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 7627,
            "range": "± 51",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 214280,
            "range": "± 480",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1949,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 493970,
            "range": "± 4581",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 496587,
            "range": "± 2031",
            "unit": "ns/iter"
          }
        ]
      },
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
          "id": "4427efcd6263d4e9f55175a46e2036b50d18b972",
          "message": "docs: check off T02.12\n\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-11-02T00:46:23Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/4427efcd6263d4e9f55175a46e2036b50d18b972"
        },
        "date": 1793614385797,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5246,
            "range": "± 231",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 5644,
            "range": "± 19",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 170933,
            "range": "± 239",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1754,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 204949023,
            "range": "± 125095216",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 789032,
            "range": "± 1499",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 762113,
            "range": "± 9278",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "name": "LordPhenixDeNetra",
            "username": "LordPhenixDeNetra",
            "email": "69590712+LordPhenixDeNetra@users.noreply.github.com"
          },
          "committer": {
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "6c1f7090aec4a3aa21340583bff190de83995ee8",
          "message": "docs: check off T02.13\n\nCo-authored-by: NetraDevLPDN <144937204+NetraDevLPDN@users.noreply.github.com>\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-11-03T04:17:33Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/6c1f7090aec4a3aa21340583bff190de83995ee8"
        },
        "date": 1793700853019,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5023,
            "range": "± 122",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 5606,
            "range": "± 83",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 171052,
            "range": "± 282",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1742,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 93919976,
            "range": "± 5699465",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 762861,
            "range": "± 12265",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 746790,
            "range": "± 2558",
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
          "id": "ebbe210aea56a487c224c588d744ace1ce682a62",
          "message": "build(deps): Bump libc from 0.2.190 to 0.2.191\n\nBumps [libc](https://github.com/rust-lang/libc) from 0.2.190 to 0.2.191.\n- [Release notes](https://github.com/rust-lang/libc/releases)\n- [Changelog](https://github.com/rust-lang/libc/blob/0.2.191/CHANGELOG.md)\n- [Commits](https://github.com/rust-lang/libc/compare/0.2.190...0.2.191)\n\n---\nupdated-dependencies:\n- dependency-name: libc\n  dependency-version: 0.2.191\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-03T21:06:14Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/ebbe210aea56a487c224c588d744ace1ce682a62"
        },
        "date": 1793786656610,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 3544,
            "range": "± 89",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 3945,
            "range": "± 297",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 167640,
            "range": "± 3830",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1285,
            "range": "± 30",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 79598064,
            "range": "± 1923840",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 559165,
            "range": "± 11165",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 533035,
            "range": "± 24838",
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
          "id": "ebbe210aea56a487c224c588d744ace1ce682a62",
          "message": "build(deps): Bump libc from 0.2.190 to 0.2.191\n\nBumps [libc](https://github.com/rust-lang/libc) from 0.2.190 to 0.2.191.\n- [Release notes](https://github.com/rust-lang/libc/releases)\n- [Changelog](https://github.com/rust-lang/libc/blob/0.2.191/CHANGELOG.md)\n- [Commits](https://github.com/rust-lang/libc/compare/0.2.190...0.2.191)\n\n---\nupdated-dependencies:\n- dependency-name: libc\n  dependency-version: 0.2.191\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-03T21:06:14Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/ebbe210aea56a487c224c588d744ace1ce682a62"
        },
        "date": 1793872837703,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 7479,
            "range": "± 278",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 7604,
            "range": "± 54",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 208796,
            "range": "± 2231",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1937,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 35505750,
            "range": "± 1731999",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 964379,
            "range": "± 10946",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 928551,
            "range": "± 9638",
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
          "id": "ebbe210aea56a487c224c588d744ace1ce682a62",
          "message": "build(deps): Bump libc from 0.2.190 to 0.2.191\n\nBumps [libc](https://github.com/rust-lang/libc) from 0.2.190 to 0.2.191.\n- [Release notes](https://github.com/rust-lang/libc/releases)\n- [Changelog](https://github.com/rust-lang/libc/blob/0.2.191/CHANGELOG.md)\n- [Commits](https://github.com/rust-lang/libc/compare/0.2.190...0.2.191)\n\n---\nupdated-dependencies:\n- dependency-name: libc\n  dependency-version: 0.2.191\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-03T21:06:14Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/ebbe210aea56a487c224c588d744ace1ce682a62"
        },
        "date": 1793959334164,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 7508,
            "range": "± 293",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 7621,
            "range": "± 61",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 212954,
            "range": "± 753",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1937,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 35520988,
            "range": "± 1092599",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 965637,
            "range": "± 7235",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 925532,
            "range": "± 8400",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}