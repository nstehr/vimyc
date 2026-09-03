rule every-construct {
  priority 800
  category economy exclusive
  because "every production in the grammar, in one rule"

  let spare = count(idle-ground-units) - 1
  let ratio = squad-ready-ratio(ground-attack)

  do       form-squad(ground-attack, Ground, 8, Attack)

  require not enemies-visible
  require cash >= 300 and power-excess < 100
  require count(powr) == 0 or count(e1) != 3
  require spare * 2 + 1 > 4 / 2
  require -spare < 0
  require ratio >= 0.75
  require exists nearest-enemy
  require can-build(Building, powr)
  require (cash > 1) and (has-role(barracks) or has-building(fact))
}

rule minimal {
  priority 1
  category combat
  do       scout
  require cash >= 1
}
