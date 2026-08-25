# vimyc

A small typed language for the rules in [Vimy](../vimy), my OpenRA bot.

```
rule build-power {
  priority 800
  category economy exclusive
  do       produce-power-plant

  require not queue-busy(Building)
  require can-build(Building, powr)
  require power-excess < 100 or count(powr) == 0
  require cash >= 300
}
```

Vimy at its core is a set of rules that get evaluated every tick of the game. This is currently implemented in Go 
using expr for handling expressions in the rules. Things like `len(idleHarvestors) > 0`. The rules are implemented in the
Go source code mostly with string formatting. 

This project aims to replace that with a small language written in Rust, mostly as an excuse for me to learn some Rust. :) 

