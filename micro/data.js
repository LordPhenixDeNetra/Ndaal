window.BENCHMARK_DATA = {
  "lastUpdate": 1795779154417,
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
        "date": 1794043395859,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 7366,
            "range": "± 160",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 7611,
            "range": "± 77",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 213275,
            "range": "± 413",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1968,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 35615638,
            "range": "± 1486991",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 963807,
            "range": "± 2528",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 926126,
            "range": "± 8773",
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
          "id": "c08ad16909071924b9b4146c85e56b96d3b103e3",
          "message": "docs: internal keys spec, plan and defects DEF-36, DEF-37 (T02.14)\n\nCo-authored-by: NetraDevLPDN <144937204+NetraDevLPDN@users.noreply.github.com>\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-11-08T03:29:09Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/c08ad16909071924b9b4146c85e56b96d3b103e3"
        },
        "date": 1794130846974,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 7445,
            "range": "± 274",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 8539,
            "range": "± 19",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 204961,
            "range": "± 1111",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1954,
            "range": "± 13",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 35568349,
            "range": "± 1291400",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 963183,
            "range": "± 2632",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 926735,
            "range": "± 7509",
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
          "id": "c08ad16909071924b9b4146c85e56b96d3b103e3",
          "message": "docs: internal keys spec, plan and defects DEF-36, DEF-37 (T02.14)\n\nCo-authored-by: NetraDevLPDN <144937204+NetraDevLPDN@users.noreply.github.com>\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-11-08T03:29:09Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/c08ad16909071924b9b4146c85e56b96d3b103e3"
        },
        "date": 1794220321796,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 7364,
            "range": "± 192",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 8560,
            "range": "± 177",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 205477,
            "range": "± 649",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1945,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 35668028,
            "range": "± 2190679",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 963073,
            "range": "± 13938",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 927211,
            "range": "± 9443",
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
          "id": "c08ad16909071924b9b4146c85e56b96d3b103e3",
          "message": "docs: internal keys spec, plan and defects DEF-36, DEF-37 (T02.14)\n\nCo-authored-by: NetraDevLPDN <144937204+NetraDevLPDN@users.noreply.github.com>\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-11-08T03:29:09Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/c08ad16909071924b9b4146c85e56b96d3b103e3"
        },
        "date": 1794306904950,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 7563,
            "range": "± 324",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 8625,
            "range": "± 210",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 204753,
            "range": "± 375",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1946,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 35396637,
            "range": "± 1104854",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 962631,
            "range": "± 6344",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 920972,
            "range": "± 11825",
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
          "id": "c08ad16909071924b9b4146c85e56b96d3b103e3",
          "message": "docs: internal keys spec, plan and defects DEF-36, DEF-37 (T02.14)\n\nCo-authored-by: NetraDevLPDN <144937204+NetraDevLPDN@users.noreply.github.com>\nSigned-off-by: LordPhenixDeNetra <69590712+LordPhenixDeNetra@users.noreply.github.com>",
          "timestamp": "2026-11-08T03:29:09Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/c08ad16909071924b9b4146c85e56b96d3b103e3"
        },
        "date": 1794391082604,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5002,
            "range": "± 79",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 6211,
            "range": "± 36",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 166505,
            "range": "± 634",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1775,
            "range": "± 16",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 103494216,
            "range": "± 17375900",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 769107,
            "range": "± 14584",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 743370,
            "range": "± 9079",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "287c0065393c132b31532a4cb43dfec0fc2bf242",
          "message": "build(deps): Bump crc32c from 0.6.8 to 0.6.9\n\nBumps [crc32c](https://github.com/zowens/crc32c) from 0.6.8 to 0.6.9.\n- [Commits](https://github.com/zowens/crc32c/compare/v0.6.8...v0.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: crc32c\n  dependency-version: 0.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-10T21:06:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/287c0065393c132b31532a4cb43dfec0fc2bf242"
        },
        "date": 1794479252735,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 7496,
            "range": "± 156",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 7607,
            "range": "± 77",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 213099,
            "range": "± 817",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1947,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 35603729,
            "range": "± 1014762",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 658627,
            "range": "± 2178",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 715790,
            "range": "± 8585",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "287c0065393c132b31532a4cb43dfec0fc2bf242",
          "message": "build(deps): Bump crc32c from 0.6.8 to 0.6.9\n\nBumps [crc32c](https://github.com/zowens/crc32c) from 0.6.8 to 0.6.9.\n- [Commits](https://github.com/zowens/crc32c/compare/v0.6.8...v0.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: crc32c\n  dependency-version: 0.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-10T21:06:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/287c0065393c132b31532a4cb43dfec0fc2bf242"
        },
        "date": 1794564864425,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5433,
            "range": "± 174",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 6118,
            "range": "± 48",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 177632,
            "range": "± 255",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1125,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 65434116,
            "range": "± 275858",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 396747,
            "range": "± 2217",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 416148,
            "range": "± 2093",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "287c0065393c132b31532a4cb43dfec0fc2bf242",
          "message": "build(deps): Bump crc32c from 0.6.8 to 0.6.9\n\nBumps [crc32c](https://github.com/zowens/crc32c) from 0.6.8 to 0.6.9.\n- [Commits](https://github.com/zowens/crc32c/compare/v0.6.8...v0.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: crc32c\n  dependency-version: 0.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-10T21:06:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/287c0065393c132b31532a4cb43dfec0fc2bf242"
        },
        "date": 1794649008997,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5446,
            "range": "± 108",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 6203,
            "range": "± 39",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 178650,
            "range": "± 781",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1124,
            "range": "± 28",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 65430745,
            "range": "± 93350",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 396566,
            "range": "± 11319",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 416107,
            "range": "± 2065",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "287c0065393c132b31532a4cb43dfec0fc2bf242",
          "message": "build(deps): Bump crc32c from 0.6.8 to 0.6.9\n\nBumps [crc32c](https://github.com/zowens/crc32c) from 0.6.8 to 0.6.9.\n- [Commits](https://github.com/zowens/crc32c/compare/v0.6.8...v0.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: crc32c\n  dependency-version: 0.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-10T21:06:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/287c0065393c132b31532a4cb43dfec0fc2bf242"
        },
        "date": 1794737025385,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5058,
            "range": "± 138",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 5648,
            "range": "± 124",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 171711,
            "range": "± 2356",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1765,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 137022106,
            "range": "± 115278826",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 540282,
            "range": "± 8523",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 580253,
            "range": "± 12050",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "287c0065393c132b31532a4cb43dfec0fc2bf242",
          "message": "build(deps): Bump crc32c from 0.6.8 to 0.6.9\n\nBumps [crc32c](https://github.com/zowens/crc32c) from 0.6.8 to 0.6.9.\n- [Commits](https://github.com/zowens/crc32c/compare/v0.6.8...v0.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: crc32c\n  dependency-version: 0.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-10T21:06:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/287c0065393c132b31532a4cb43dfec0fc2bf242"
        },
        "date": 1794824281773,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 7501,
            "range": "± 227",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 7603,
            "range": "± 605",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 213331,
            "range": "± 578",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1973,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 35769497,
            "range": "± 1479702",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 658620,
            "range": "± 1981",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 712441,
            "range": "± 10296",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "287c0065393c132b31532a4cb43dfec0fc2bf242",
          "message": "build(deps): Bump crc32c from 0.6.8 to 0.6.9\n\nBumps [crc32c](https://github.com/zowens/crc32c) from 0.6.8 to 0.6.9.\n- [Commits](https://github.com/zowens/crc32c/compare/v0.6.8...v0.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: crc32c\n  dependency-version: 0.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-10T21:06:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/287c0065393c132b31532a4cb43dfec0fc2bf242"
        },
        "date": 1794914303485,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5216,
            "range": "± 73",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 6186,
            "range": "± 34",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 177612,
            "range": "± 1695",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1144,
            "range": "± 24",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 65434703,
            "range": "± 297456",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 396269,
            "range": "± 7189",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 416849,
            "range": "± 3584",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "287c0065393c132b31532a4cb43dfec0fc2bf242",
          "message": "build(deps): Bump crc32c from 0.6.8 to 0.6.9\n\nBumps [crc32c](https://github.com/zowens/crc32c) from 0.6.8 to 0.6.9.\n- [Commits](https://github.com/zowens/crc32c/compare/v0.6.8...v0.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: crc32c\n  dependency-version: 0.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-10T21:06:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/287c0065393c132b31532a4cb43dfec0fc2bf242"
        },
        "date": 1794999023201,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 6526,
            "range": "± 69",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 7303,
            "range": "± 58",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 203265,
            "range": "± 3307",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1780,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 35570443,
            "range": "± 972682",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 660670,
            "range": "± 3915",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 664640,
            "range": "± 6485",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "287c0065393c132b31532a4cb43dfec0fc2bf242",
          "message": "build(deps): Bump crc32c from 0.6.8 to 0.6.9\n\nBumps [crc32c](https://github.com/zowens/crc32c) from 0.6.8 to 0.6.9.\n- [Commits](https://github.com/zowens/crc32c/compare/v0.6.8...v0.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: crc32c\n  dependency-version: 0.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-10T21:06:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/287c0065393c132b31532a4cb43dfec0fc2bf242"
        },
        "date": 1795085529159,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5189,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 6162,
            "range": "± 124",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 177683,
            "range": "± 361",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1133,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 65435709,
            "range": "± 20602",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 396592,
            "range": "± 2753",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 412934,
            "range": "± 1812",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "287c0065393c132b31532a4cb43dfec0fc2bf242",
          "message": "build(deps): Bump crc32c from 0.6.8 to 0.6.9\n\nBumps [crc32c](https://github.com/zowens/crc32c) from 0.6.8 to 0.6.9.\n- [Commits](https://github.com/zowens/crc32c/compare/v0.6.8...v0.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: crc32c\n  dependency-version: 0.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-10T21:06:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/287c0065393c132b31532a4cb43dfec0fc2bf242"
        },
        "date": 1795171285593,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 3779,
            "range": "± 134",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 3921,
            "range": "± 151",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 169015,
            "range": "± 4135",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1294,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 77743196,
            "range": "± 1146869",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 367136,
            "range": "± 15188",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 408503,
            "range": "± 7466",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "287c0065393c132b31532a4cb43dfec0fc2bf242",
          "message": "build(deps): Bump crc32c from 0.6.8 to 0.6.9\n\nBumps [crc32c](https://github.com/zowens/crc32c) from 0.6.8 to 0.6.9.\n- [Commits](https://github.com/zowens/crc32c/compare/v0.6.8...v0.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: crc32c\n  dependency-version: 0.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-10T21:06:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/287c0065393c132b31532a4cb43dfec0fc2bf242"
        },
        "date": 1795256642130,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 7498,
            "range": "± 187",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 7611,
            "range": "± 125",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 212850,
            "range": "± 972",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1931,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 35507556,
            "range": "± 699444",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 660109,
            "range": "± 16325",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 714492,
            "range": "± 5962",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "287c0065393c132b31532a4cb43dfec0fc2bf242",
          "message": "build(deps): Bump crc32c from 0.6.8 to 0.6.9\n\nBumps [crc32c](https://github.com/zowens/crc32c) from 0.6.8 to 0.6.9.\n- [Commits](https://github.com/zowens/crc32c/compare/v0.6.8...v0.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: crc32c\n  dependency-version: 0.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-10T21:06:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/287c0065393c132b31532a4cb43dfec0fc2bf242"
        },
        "date": 1795343280686,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 7513,
            "range": "± 183",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 7592,
            "range": "± 85",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 213214,
            "range": "± 6413",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1912,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 35666900,
            "range": "± 681566",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 660675,
            "range": "± 8100",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 714089,
            "range": "± 28738",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "287c0065393c132b31532a4cb43dfec0fc2bf242",
          "message": "build(deps): Bump crc32c from 0.6.8 to 0.6.9\n\nBumps [crc32c](https://github.com/zowens/crc32c) from 0.6.8 to 0.6.9.\n- [Commits](https://github.com/zowens/crc32c/compare/v0.6.8...v0.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: crc32c\n  dependency-version: 0.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-10T21:06:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/287c0065393c132b31532a4cb43dfec0fc2bf242"
        },
        "date": 1795433285527,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5228,
            "range": "± 161",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 6228,
            "range": "± 42",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 179035,
            "range": "± 955",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1155,
            "range": "± 16",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 65435874,
            "range": "± 306675",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 399892,
            "range": "± 3359",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 416755,
            "range": "± 2569",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "287c0065393c132b31532a4cb43dfec0fc2bf242",
          "message": "build(deps): Bump crc32c from 0.6.8 to 0.6.9\n\nBumps [crc32c](https://github.com/zowens/crc32c) from 0.6.8 to 0.6.9.\n- [Commits](https://github.com/zowens/crc32c/compare/v0.6.8...v0.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: crc32c\n  dependency-version: 0.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-10T21:06:00Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/287c0065393c132b31532a4cb43dfec0fc2bf242"
        },
        "date": 1795525332062,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5425,
            "range": "± 90",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 6182,
            "range": "± 36",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 178011,
            "range": "± 707",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1135,
            "range": "± 36",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 65439038,
            "range": "± 38510",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 396069,
            "range": "± 4128",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 417873,
            "range": "± 4739",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "c0067888da62c1a120a34b7ecabf0d1f84a88f22",
          "message": "build(deps): Bump clap from 4.6.7 to 4.6.9\n\nBumps [clap](https://github.com/clap-rs/clap) from 4.6.7 to 4.6.9.\n- [Release notes](https://github.com/clap-rs/clap/releases)\n- [Changelog](https://github.com/clap-rs/clap/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/clap-rs/clap/compare/clap_complete-v4.6.7...clap_complete-v4.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: clap\n  dependency-version: 4.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-24T21:05:59Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/c0067888da62c1a120a34b7ecabf0d1f84a88f22"
        },
        "date": 1795607501918,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5200,
            "range": "± 42",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 6176,
            "range": "± 31",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 177943,
            "range": "± 1131",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1135,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 65445734,
            "range": "± 526369",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 398434,
            "range": "± 2558",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 415050,
            "range": "± 2666",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "c0067888da62c1a120a34b7ecabf0d1f84a88f22",
          "message": "build(deps): Bump clap from 4.6.7 to 4.6.9\n\nBumps [clap](https://github.com/clap-rs/clap) from 4.6.7 to 4.6.9.\n- [Release notes](https://github.com/clap-rs/clap/releases)\n- [Changelog](https://github.com/clap-rs/clap/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/clap-rs/clap/compare/clap_complete-v4.6.7...clap_complete-v4.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: clap\n  dependency-version: 4.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-24T21:05:59Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/c0067888da62c1a120a34b7ecabf0d1f84a88f22"
        },
        "date": 1795693185083,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5223,
            "range": "± 137",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 6178,
            "range": "± 44",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 178575,
            "range": "± 762",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1136,
            "range": "± 39",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 65439795,
            "range": "± 338204",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 398152,
            "range": "± 2187",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 416306,
            "range": "± 2078",
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
            "name": "THIOR",
            "username": "NetraDevLPDN",
            "email": "144937204+NetraDevLPDN@users.noreply.github.com"
          },
          "id": "c0067888da62c1a120a34b7ecabf0d1f84a88f22",
          "message": "build(deps): Bump clap from 4.6.7 to 4.6.9\n\nBumps [clap](https://github.com/clap-rs/clap) from 4.6.7 to 4.6.9.\n- [Release notes](https://github.com/clap-rs/clap/releases)\n- [Changelog](https://github.com/clap-rs/clap/blob/main/CHANGELOG.md)\n- [Commits](https://github.com/clap-rs/clap/compare/clap_complete-v4.6.7...clap_complete-v4.6.9)\n\n---\nupdated-dependencies:\n- dependency-name: clap\n  dependency-version: 4.6.9\n  dependency-type: direct:production\n  update-type: version-update:semver-patch\n...\n\nSigned-off-by: dependabot[bot] <support@github.com>",
          "timestamp": "2026-11-24T21:05:59Z",
          "url": "https://github.com/LordPhenixDeNetra/Ndaal/commit/c0067888da62c1a120a34b7ecabf0d1f84a88f22"
        },
        "date": 1795779153739,
        "tool": "cargo",
        "benches": [
          {
            "name": "varint/encode",
            "value": 5021,
            "range": "± 141",
            "unit": "ns/iter"
          },
          {
            "name": "varint/decode",
            "value": 5649,
            "range": "± 35",
            "unit": "ns/iter"
          },
          {
            "name": "crockford/decode",
            "value": 174463,
            "range": "± 1998",
            "unit": "ns/iter"
          },
          {
            "name": "runtime/round_trip",
            "value": 1749,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "wal/append",
            "value": 106895429,
            "range": "± 38972406",
            "unit": "ns/iter"
          },
          {
            "name": "wal/encode",
            "value": 537837,
            "range": "± 1235",
            "unit": "ns/iter"
          },
          {
            "name": "wal/decode",
            "value": 578805,
            "range": "± 8990",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}