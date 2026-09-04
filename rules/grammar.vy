param aggression: float
param group-size: int

def reserve(cost: int, headroom: int,) =
  cash >= cost and (has-role(radar) or cash >= cost + headroom)

def commit() = lerpf(0.6, 1.0, 1.0 - aggression)

rule every-construct {
  priority trunc(max(200.0, min(900.0, lerp(200, 400, aggression))))
  category economy exclusive
  because "every production in the grammar, in one rule"

  let spare = count(idle-ground-units) - 1
  let ratio = squad-ready-ratio(ground-attack)

  do       form-squad(ground-attack, Ground, group-size, Attack,)

  require not enemies-visible
  require cash >= 300 and power-excess < 100
  require count(powr) == 0 or count(e1) != 3
  require spare * 2 + 1 > 4 / 2
  require -spare < 0
  require ratio >= 0.75
  require exists nearest-enemy
  require can-build(Building, powr)
  require (cash > 1) and (has-role(barracks) or has-building(fact))
  require reserve(300, 1000)
  require ratio >= round2(commit())
  require aggression > 0.1
  require select(aggression > 0.5, 1.0, 0.0) > 0.5
}

rule minimal {
  priority 1
  category combat
  do       scout
  require cash >= 1
}
