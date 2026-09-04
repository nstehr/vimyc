param capture-priority: float
param transport-assault: float
param vehicle-weight: float
param tech-priority: float
param superweapon-priority: float
param infantry-weight: float
param aggression: float

def reserves(cost: int) =
  cash >= cost
  and (vehicle-weight <= 0.1 or has-role(radar) or cash >= cost + 1000)
  and (vehicle-weight <= 0.2 or has-role(war-factory) or not has-role(radar) or cash >= cost + 2000)
  and (tech-priority <= 0.4 or has-role(tech-center) or not has-role(radar) or cash >= cost + 1500)
  and (superweapon-priority <= 0.4 or has-role(missile-silo) or has-role(iron-curtain)
       or not has-role(tech-center) or cash >= cost + 2500)

rule deploy-mcv {
  priority 1000
  category setup exclusive
  do deploy-mcv
  require has-unit(mcv)
  require not has-role(construction-yard)
}

rule recover-mcv {
  priority 950
  category setup exclusive
  do produce-mcv
  require not has-role(construction-yard)
  require not has-unit(mcv)
  require has-role(war-factory)
  require not queue-busy(Vehicle)
  require can-build(Vehicle, mcv)
  require cash >= 1000
}

rule place-ready-building {
  priority 900
  category economy exclusive
  do place-building
  require queue-ready(Building)
}

rule place-ready-defense {
  priority 895
  category defense exclusive
  do place-defense
  require queue-ready(Defense)
}

rule unblock-war-factory-egress {
  priority 892
  category vehicle-maintenance exclusive
  do unblock-war-factory-egress
  require has-role(war-factory)
  require queue-ready(Vehicle)
}

rule cancel-stuck-aircraft {
  priority 891
  category aircraft-maintenance exclusive
  do cancel-stuck-aircraft
  require queue-ready(Aircraft)
}

rule capture-building {
  priority 850
  category capture
  do capture-building
  require capturable-count > 0
  require count(idle-engineers) > 0
  require not can-build-transport() or engineer-near-capturable()
}

rule deliver-apc-to-target {
  priority 847
  category capture
  because "an aggressive capture doctrine sends a loaded APC without waiting for a target"
  do unload-apc-near-target
  require capture-priority >= 0.3 or capturable-count > 0
  require count(idle-engineer-loaded-apcs) > 0
}

rule load-engineer-into-apc {
  priority 845
  category capture
  do load-engineer-into-apc
  require role-count(engineer) > 0 or transport-count > 0
  require count(idle-engineers) > 0
  require count(idle-empty-apcs) > 0
  require not engineer-near-capturable()
}

rule produce-apc {
  priority 470
  category produce-vehicle exclusive
  do produce-apc
  require capture-priority > 0.1
  require capture-priority >= 0.3 or capturable-count > 0
  require role-count(engineer) > 0
  require has-role(war-factory)
  require not queue-busy(Vehicle)
  require can-build-transport()
  require transport-count < 1
  require cash >= 800
}

rule produce-engineer {
  priority 450
  category produce-infantry exclusive
  do produce-engineer
  require capture-priority > 0.1
  require capture-priority >= 0.3 or capturable-count > 0
  require not queue-busy(Infantry)
  require can-build-role(engineer)
  require capture-priority >= 0.3 or role-count(engineer) < capturable-count
  require role-count(engineer) < lerp(1, 3, capture-priority)
  require cash >= 500
}

rule produce-capture-defense-infantry {
  priority 440
  category produce-infantry exclusive
  do produce-infantry
  require capture-priority > 0.1
  require has-role(barracks)
  require not queue-busy(Infantry)
  require can-build(Infantry, e1)
  require count(e1) < 3
  require cash >= 100
}

rule produce-assault-apc {
  priority lerp(475, 490, transport-assault)
  category produce-vehicle exclusive
  do produce-apc
  require transport-assault > 0.1
  require has-role(war-factory)
  require not queue-busy(Vehicle)
  require can-build-transport()
  require transport-count < lerp(1, 3, transport-assault)
  require reserves(800)
}

rule deliver-assault-apc {
  priority 840
  category transport
  do deliver-assault-apc
  require transport-assault > 0.1
  require transport-assault >= 0.3 or has-enemy-intel()
  require count(idle-combat-loaded-apcs) > 0
}

rule load-assault-infantry {
  priority 838
  category transport
  do load-combat-infantry
  require transport-assault > 0.1
  require count(idle-combat-infantry) > 0
  require count(idle-empty-apcs) > 0
}

rule rebuild-power-plant {
  priority 840
  category rebuild exclusive
  do produce-power-plant
  require lost-role(power-plant)
  require power-excess < 0
  require not queue-busy(Building)
  require can-build-role(power-plant)
  require cash >= 300
}

rule rebuild-advanced-power {
  priority 835
  category rebuild exclusive
  do produce-advanced-power
  require lost-role(advanced-power)
  require power-excess < 0
  require not queue-busy(Building)
  require can-build-role(advanced-power)
  require cash >= 500
}

rule rebuild-harvester {
  priority 830
  category rebuild exclusive
  do produce-harvester
  require has-role(refinery)
  require role-count(harvester) == 0
  require not queue-busy(Vehicle)
  require can-build-role(harvester)
  require cash >= 600
}

rule rebuild-refinery {
  priority 825
  category rebuild exclusive
  do produce-refinery
  require lost-role(refinery)
  require not queue-busy(Building)
  require can-build-role(refinery)
  require cash >= 500
}

rule rebuild-barracks {
  priority 820
  category rebuild exclusive
  do produce-barracks
  require lost-role(barracks)
  require not queue-busy(Building)
  require can-build-role(barracks)
  require cash >= 200
}

rule rebuild-war-factory {
  priority 815
  category rebuild exclusive
  do produce-war-factory
  require lost-role(war-factory)
  require not queue-busy(Building)
  require can-build-role(war-factory)
  require cash >= 1000
}

rule rebuild-radar {
  priority 810
  category rebuild exclusive
  do produce-radar
  require lost-role(radar)
  require not queue-busy(Building)
  require not queue-producing-role(radar)
  require can-build-role(radar)
  require cash >= 500
}

rule rebuild-tech-center {
  priority 805
  category rebuild exclusive
  do produce-tech-center
  require lost-role(tech-center)
  require not queue-busy(Building)
  require can-build-role(tech-center)
  require has-role(radar)
  require cash >= 1000
}

rule rebuild-airfield {
  priority 800
  category rebuild exclusive
  do produce-airfield
  require lost-role(airfield)
  require not queue-busy(Building)
  require can-build-role(airfield)
  require cash >= 300
}

rule rebuild-naval-yard {
  priority 800
  category rebuild exclusive
  do produce-naval-yard
  require map-has-water()
  require lost-role(naval-yard)
  require not queue-busy(Building)
  require can-build-role(naval-yard)
  require cash >= 300
}

rule rebuild-service-depot {
  priority 795
  category rebuild exclusive
  do produce-service-depot
  require lost-role(service-depot)
  require not queue-busy(Building)
  require can-build-role(service-depot)
  require cash >= 800
}

rule rebuild-missile-silo {
  priority 790
  category rebuild exclusive
  do produce-missile-silo
  require lost-role(missile-silo)
  require not queue-busy(Defense)
  require can-build-role(missile-silo)
  require cash >= 2500
}

rule rebuild-iron-curtain {
  priority 785
  category rebuild exclusive
  do produce-iron-curtain
  require lost-role(iron-curtain)
  require not queue-busy(Defense)
  require can-build-role(iron-curtain)
  require cash >= 2500
}

rule rebuild-kennel {
  priority 780
  category rebuild exclusive
  do produce-kennel
  require lost-role(kennel)
  require not queue-busy(Building)
  require can-build-role(kennel)
  require cash >= 200
}

rule defend-critical-building {
  priority 360
  category combat
  do defend-critical-building
  require critical-building-under-attack()
  require count(near-base-ground-units) > 0
}

rule scramble-base-defense {
  priority 350
  category combat
  do defend-base
  require base-under-attack()
  require count(unassigned-idle-ground) > 0
}

rule scramble-naval-defense {
  priority 350
  category naval-combat
  do naval-defend-base
  require map-has-water()
  require base-under-attack()
  require count(idle-naval-units) > 0
}

rule emergency-base-defense {
  priority 349
  category emergency-defense
  do emergency-defend-base
  require base-under-attack()
  require count(idle-ground-units) == 0
  require count(near-base-ground-units) > 0
}

rule repair-buildings {
  priority 200
  category maintenance
  do repair-buildings
  require count(damaged-buildings) > 0
}

rule return-idle-harvesters {
  priority 100
  category harvester
  do send-harvesters
  require count(idle-harvesters) > 0
}
