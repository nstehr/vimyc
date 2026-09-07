def committed() = squad-ready-ratio(ground-attack) >= 0.6

rule build-power {
  priority 800
  category economy exclusive
  because "a rule set for the compiler's own tests, not Vimy's strategy"
  do produce-power-plant
  require not queue-busy(Building)
  require can-build(Building, powr)
  require power-excess < 100 or count(powr) == 0
  require cash >= 300
}

rule produce-infantry {
  priority 500
  category produce-infantry exclusive
  do produce-infantry
  require has-role(barracks)
  require not queue-busy(Infantry)
  require count(e1) < 10
  require cash >= 100
}

rule defend-base {
  priority 400
  category combat
  do defend-base
  require base-under-attack()
  require count(idle-ground-units) >= 2
}

rule attack-idle-units {
  priority 300
  category combat
  do attack-move-ground
  require count(idle-ground-units) >= 5
  require exists nearest-enemy
}

rule squad-attack {
  priority 250
  category ground-attack-choice exclusive
  do squad-attack-move(ground-attack)
  require squad-exists(ground-attack)
  require committed()
  require exists nearest-enemy
}

rule return-idle-harvesters {
  priority 100
  category harvester
  do send-harvesters
  require count(idle-harvesters) > 0
}
