# Messages of the ndaal command line (ADR-029). Keys: cli-*.
# Every key here exists in every other language (checked by the tests).

cli-about = Ndaal: key-value store, vector search and object storage in one engine.
cli-help-usage = Usage
cli-help-commands = Commands
cli-help-options = Options

cli-value-file = FILE
cli-value-lang = LANG
cli-value-setting = KEY=VALUE
cli-value-dir = DIR

cli-opt-config = Configuration file (TOML)
cli-opt-lang = Language of the messages: en, fr
cli-opt-set = Set a configuration parameter: section.key=value
cli-opt-data-dir = Data directory (node.data_dir)
cli-opt-help = Print this help
cli-opt-version = Print the version

cli-cmd-server = Start a node
cli-cmd-admin = Administer a node or a cluster
cli-cmd-backup = Back up a node
cli-cmd-restore = Restore a backup
cli-cmd-bench = Measure performance
cli-cmd-config = Inspect the configuration
cli-cmd-config-reference = List every parameter with its type, default and variable
cli-cmd-config-show = Print the effective configuration
cli-cmd-doctor = Check the installation

cli-error-unknown-command = Unknown command: { $name }
cli-error-unknown-option = Unknown option: { $name }
cli-error-invalid-value = Missing or invalid value for { $name }
cli-error-usage = Invalid command line
cli-error-hint = Run ndaal --help for the commands and options.
cli-cmd-health = Check that a node is ready (for container health checks)
cli-data-dir-error = Cannot create the data directory { $path }: { $error }
cli-listen-error = Cannot listen on { $addr }: { $error }
cli-health-not-ready = The node at { $addr } is not ready: { $reason }
cli-health-unreachable = cannot connect ({ $error })
cli-health-timeout = no answer within 5 seconds
cli-health-status = HTTP status { $status }
cli-health-invalid = the answer is not HTTP
cli-not-available = The command { $command } is not available yet.
cli-set-syntax = Invalid value for --set: { $value } (expected section.key=value).
cli-config-read-error = Cannot read the configuration file { $path }: { $error }
cli-config-too-large = The configuration file { $path } is larger than { $max } bytes.
cli-config-not-utf8 = The configuration file { $path } is not UTF-8 text.
cli-config-syntax = Invalid configuration file (TOML): { $error }
cli-config-unknown = Unknown configuration key { $key } (source: { $origin }).
cli-config-invalid = Invalid value for { $key } (source: { $origin }): expected { $expected }.
cli-origin-default = defaults
cli-origin-file = configuration file
cli-origin-env = environment
cli-origin-command-line = command line
cli-expected-path = a non-empty path
cli-expected-text = text
cli-expected-choice = one of { $choices }
cli-expected-addr = an address such as 127.0.0.1:8080
cli-expected-log-level = a level ({ $levels }) or a list of target=level
cli-expected-bool = true or false
cli-expected-duration = a duration such as 0, 250us, 10ms or 2s (at most 10s)
cli-expected-count = auto or a number from 0 to 4096
cli-config-public-without-auth = { $key } (source: { $origin }) listens on a non-local address without authentication. Set auth.allow_insecure = true only if this is intended.
cli-expected-table = a [section] table
cli-expected-scalar = a single value, not a list or a table
cli-runtime-error = Cannot start: { $error }
