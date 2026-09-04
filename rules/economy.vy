param economy-priority: float
param tech-priority: float

rule build-power {
  priority 800
  category economy exclusive
  do produce-power-plant
  require not queue-busy(Building)
  require can-build-role(power-plant)
  require power-excess < 0 or role-count(power-plant) == 0
  require cash >= lerp(500, 200, economy-priority)
}

rule build-refinery {
  priority 750
  category economy exclusive
  because "the first refinery is the whole economy, so it outranks everything but power"
  do produce-refinery
  require not queue-busy(Building)
  require can-build-role(refinery)
  require role-count(refinery) < 1
  require cash >= lerp(2000, 800, economy-priority)
}

rule build-second-refinery {
  priority lerp(560, 700, economy-priority)
  category economy exclusive
  do produce-refinery
  require economy-priority > 0.1
  require not is-rushed()
  require not queue-busy(Building)
  require can-build-role(refinery)
  require role-count(refinery) == 1
  require has-role(barracks) or has-role(war-factory)
  require cash >= lerp(1500, 500, economy-priority)
}

rule build-extra-refinery {
  priority lerp(520, 680, economy-priority)
  category economy exclusive
  do produce-refinery
  require economy-priority > 0.1
  require lerp(1, 5, economy-priority) > 2
  require not is-rushed()
  require not queue-busy(Building)
  require can-build-role(refinery)
  require role-count(refinery) >= 2
  require role-count(refinery) < lerp(1, 5, economy-priority)
  require has-role(barracks) or has-role(war-factory)
  require cash >= lerp(2000, 800, economy-priority)
}

rule build-advanced-power {
  priority 790
  category economy exclusive
  do produce-advanced-power
  require economy-priority > 0.3 or tech-priority > 0.5
  require not is-rushed()
  require not queue-busy(Building)
  require can-build-role(advanced-power)
  require power-excess < 50
  require cash >= 500
}

rule build-ore-silo {
  priority 300
  category economy exclusive
  do produce-ore-silo
  require economy-priority > 0.5
  require not is-rushed()
  require not queue-busy(Building)
  require can-build-role(ore-silo)
  require resources-near-cap()
  require role-count(ore-silo) < lerp(0, 2, economy-priority)
  require cash >= 150
}

rule produce-extra-harvester {
  priority 510
  category produce-vehicle exclusive
  because "above every combat-vehicle rule, so the exclusive queue cannot starve income"
  do produce-harvester
  require economy-priority > 0.5
  require has-role(refinery)
  require has-role(war-factory)
  require not queue-busy(Vehicle)
  require can-build-role(harvester)
  require role-count(harvester) < role-count(refinery) + 1
  require cash >= 1400
}
