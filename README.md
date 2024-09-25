# jstream

Enumerate the paths through a JSON document.

This project is very much like [gron](https://github.com/tomnomnom/gron) or my other project, [jindex](https://github.com/ckampfe/jindex), but this project is much faster and uses *much* less memory as it parses the input bytes in a streaming fashion via [aws-smithy-json](https://crates.io/crates/aws-smithy-json).

Right now, it only outputs JSON paths (see below), but the backend is fully extendable, so any kind of output formatter can be written by implementing a trait.

See [src/path_value_writer/json_pointer.rs](https://github.com/ckampfe/jstream/blob/main/src/path_value_writer/json_pointer.rs) for what this looks like.

## Installation

Latest unstable (HEAD) release from source:

```
$ cargo install --git https://github.com/ckampfe/jstream
```

## Examples

You can pass JSON through stdin or a file (not shown):

```
$ echo '{
  "a": 1,
  "b": 2,
  "c": ["x", "y", "z"],
  "d": {"e": {"f": [{}, 9, "g"]}}
}' | jstream    
/a      1
/b      2
/c/0    "x"
/c/1    "y"
/c/2    "z"
/d/e/f/1        9
/d/e/f/2        "g"

```

## Command-line interface

```
$ jstream -h
Enumerate the paths through a JSON document

Usage: jstream [JSON_LOCATION]

Arguments:
  [JSON_LOCATION]  A JSON file path

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## Path output order

`jstream` makes *no guarantees at all* about the order in which paths are output. Paths may appear depth-first, breadth-first, or any other order at all relative to their position in the input JSON document. Further, *any ordering is not guaranteed to be stable from one version to the next*,
as it may change to aid the implementation of new optimizations.
If a stable order is important, I recommend using `sort` or some other after-the-fact mechanism, as the set of paths output from a given input document are guaranteed to be stable over time.

## Output notes

The command line interface for `jstream` (`main.rs`) currently only outputs paths for singular JSON values (strings, numbers, booleans, and null). It does *not* emit paths for arrays or objects, even those that are empty. See above for an example of this behavior.

The library (`lib.rs`), however, is able to emit paths for empty arrays and empty objects.

This is subject to change.

## Performance

On my machine, on a typical (large) payload, `jstream` runs at ~200MB/s.

```
$ /bin/ls -la ~/code/citylots.json
-rw-rw-rw-@ 1 clark  staff  189778220 Jun 28  2021 /Users/clark/code/citylots.json

$ hyperfine -w3 -r9 --output=null "jstream ~/code/citylots.json"
Benchmark 1: jstream ~/code/citylots.json
  Time (mean ± σ):     910.6 ms ±   2.4 ms    [User: 838.0 ms, System: 56.5 ms]
  Range (min … max):   905.9 ms … 914.7 ms    9 runs
```

And `jstream` has barely any memory overhead compared to the size of the input due to its streaming nature. The following is on MacOS, so the maximum resident set size number of 191692800 is in bytes, meaning on this 181MB [citylots.json](https://github.com/zemirco/sf-city-lots-json/blob/master/citylots.json) input, the overhead is only ~1.8MB.

```
at [ 16:41:27 ] ➜ /usr/bin/time -l jstream ~/code/citylots.json >/dev/null
        0.96 real         0.84 user         0.07 sys
           191692800  maximum resident set size
                   0  average shared memory size
                   0  average unshared data size
                   0  average unshared stack size
               11817  page reclaims
                   2  page faults
                   0  swaps
                   0  block input operations
                   0  block output operations
                   0  messages sent
                   0  messages received
                   0  signals received
                   0  voluntary context switches
                  32  involuntary context switches
         14942138146  instructions retired
          2917720211  cycles elapsed
           191202560  peak memory footprint
```

To run the included microbenchmarks:

```
# install the benchmark runner
$ cargo install cargo-criterion
```

```
# clone the project
$ git clone https://github.com/ckampfe/jstream
```

```
# run the benchmarks
$ cd jstream
$ cargo criterion
```
