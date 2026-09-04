param capture-priority: float
param transport-assault: float
param vehicle-weight: float
param tech-priority: float
param superweapon-priority: float
param infantry-weight: float
param aggression: float
param economy-priority: float
param air-weight: float
param naval-weight: float
param ground-defense-priority: float
param air-defense-priority: float
param prefers-radar-gated-primary: int
param prefers-v2-launcher: int
param prefers-artillery: int
param specialized-infantry-weight: float
param scout-priority: float
param prefers-flamethrower: int
param prefers-shock-trooper: int
param specialist-infantry-first: int
param siege-vehicle-first: int
param tech-naval-first: int
param commit-ratio: float
param base-defense-floor: int
param ground-attack-group-size: int
param air-attack-group-size: int
param naval-attack-group-size: int

def reserves(cost: int) =
  cash >= cost
  and (vehicle-weight <= 0.1 or has-role(radar) or cash >= cost + 1000)
  and (vehicle-weight <= 0.2 or has-role(war-factory) or not has-role(radar) or cash >= cost + 2000)
  and (tech-priority <= 0.4 or has-role(tech-center) or not has-role(radar) or cash >= cost + 1500)
  and (superweapon-priority <= 0.4 or has-role(missile-silo) or has-role(iron-curtain)
       or not has-role(tech-center) or cash >= cost + 2500)

def any-ground-defense-buildable() =
  can-build-role(pillbox) or can-build-role(camo-pillbox) or can-build-role(turret)
  or can-build-role(flame-tower) or can-build-role(tesla-coil)

def ground-defense-count() =
  role-count(pillbox) + role-count(camo-pillbox) + role-count(turret)
  + role-count(flame-tower) + role-count(tesla-coil)

def defense-cap() = lerp(2, 10, ground-defense-priority)

def war-factory-base() = lerp(580, 730, vehicle-weight)

def affordable(cost: int) =
  cash >= cost
  and (vehicle-weight <= 0.1 or has-role(radar) or cash >= cost + 1000)

def infantry-base() = trunc(select(specialist-infantry-first > 0, 490.0, 500.0))
def specialist-base() = trunc(select(specialist-infantry-first > 0, 500.0, 490.0))

def scaled-reserves(cost: int) =
  cash >= cost
  and (vehicle-weight <= 0.1
       or trunc(1000.0 * select(vehicle-weight >= 0.4, 1.0 - vehicle-weight, 1.0)) <= 0
       or has-role(radar)
       or cash >= cost + trunc(1000.0 * select(vehicle-weight >= 0.4, 1.0 - vehicle-weight, 1.0)))
  and (vehicle-weight <= 0.2
       or trunc(2000.0 * select(vehicle-weight >= 0.4, 1.0 - vehicle-weight, 1.0)) <= 0
       or has-role(war-factory) or not has-role(radar)
       or cash >= cost + trunc(2000.0 * select(vehicle-weight >= 0.4, 1.0 - vehicle-weight, 1.0)))
  and (tech-priority <= 0.4
       or trunc(1500.0 * select(vehicle-weight >= 0.4, 1.0 - vehicle-weight, 1.0)) <= 0
       or has-role(tech-center) or not has-role(radar)
       or cash >= cost + trunc(1500.0 * select(vehicle-weight >= 0.4, 1.0 - vehicle-weight, 1.0)))
  and (superweapon-priority <= 0.4
       or trunc(2500.0 * select(vehicle-weight >= 0.4, 1.0 - vehicle-weight, 1.0)) <= 0
       or has-role(missile-silo) or has-role(iron-curtain) or not has-role(tech-center)
       or cash >= cost + trunc(2500.0 * select(vehicle-weight >= 0.4, 1.0 - vehicle-weight, 1.0)))

def infantry-reserves(cost: int) =
  reserves(cost)
  and (vehicle-weight <= 0.1 or aggression >= 0.3
       or has-role(war-factory) or cash >= cost + 2000)
  and (vehicle-weight <= 0.2
       or trunc(800.0 * max(0.0, vehicle-weight - infantry-weight)) <= 0
       or combat-vehicle-count >= lerp(3, 10, vehicle-weight)
       or cash >= cost + trunc(800.0 * max(0.0, vehicle-weight - infantry-weight)))

def activation() =
  select(commit-ratio > 0.0, commit-ratio, lerpf(0.6, 1.0, 1.0 - aggression))

def attack-priority() = lerp(200, 400, aggression)

def defend-priority() = lerp(350, 500, ground-defense-priority)

def ground-form-threshold() = trunc(max(3.0, ground-attack-group-size * 6 / 10))

def defense-floor-holds() =
  base-defense-floor <= 0
  or role-count(pillbox) + role-count(camo-pillbox) + role-count(turret)
     + role-count(flame-tower) + role-count(tesla-coil) >= base-defense-floor

def retreat-threshold() = lerpf(0.5, 0.15, aggression)
def retreat-priority() = lerp(380, 450, 1.0 - aggression)
def leash() = lerpf(0.25, 0.5, aggression)

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

rule build-radar {
  priority trunc(select(prefers-radar-gated-primary > 0, 710.0, 570.0))
  category economy exclusive
  because "radar is the tech gate for vehicles, aircraft and naval"
  do produce-radar
  require vehicle-weight > 0.1 or air-weight > 0.1 or naval-weight > 0.1 or tech-priority > 0.3
  require not is-rushed()
  require not queue-busy(Building)
  require can-build-role(radar)
  require not has-role(radar)
  require not queue-producing-role(radar)
  require has-role(barracks) or has-role(war-factory)
  require power-excess >= 0
  require cash >= 1000
}

rule build-barracks {
  priority trunc(max(745.0, max(lerp(600, 700, infantry-weight),
                                select(ground-defense-priority > 0.2,
                                       lerp(600, 700, ground-defense-priority), 0.0))))
  category economy exclusive
  because "barracks unblocks the whole infantry queue, so it outranks radar"
  do produce-barracks
  require infantry-weight > 0.1 or ground-defense-priority > 0.2
  require not queue-busy(Building)
  require can-build-role(barracks)
  require not has-role(barracks)
  require power-excess >= 0
  require cash >= 300
}

rule build-war-factory {
  priority trunc(max(select(naval-weight > 0.1 or air-weight > 0.1, 685.0, 0.0),
                     max(select(prefers-v2-launcher > 0 or prefers-artillery > 0, 720.0, 0.0),
                         select(transport-assault > 0.2,
                                max(war-factory-base(),
                                    lerp(600, 700, transport-assault))
                                  + lerp(0, 40, transport-assault),
                                war-factory-base()))))
  category economy exclusive
  because "vehicles are universally useful, so the war factory precedes naval and air yards"
  do produce-war-factory
  require vehicle-weight > 0.1
  require not queue-busy(Building)
  require can-build-role(war-factory)
  require not has-role(war-factory)
  require power-excess >= 0
  require cash >= trunc(select(transport-assault > 0.2,
                               min(lerp(2500, 2000, vehicle-weight),
                                   lerp(2200, 2000, transport-assault)),
                               lerp(2500, 2000, vehicle-weight)))
}

rule build-barracks-prereq {
  priority 600
  category economy exclusive
  because "radar needs a barracks, and without this the two can deadlock"
  do produce-barracks
  require vehicle-weight > 0.1 or air-weight > 0.1 or naval-weight > 0.1 or tech-priority > 0.3
  require not (infantry-weight > 0.1 or ground-defense-priority > 0.2)
  require vehicle-weight <= 0.1
  require not queue-busy(Building)
  require can-build-role(barracks)
  require not has-role(barracks)
  require power-excess >= 0
  require cash >= 300
}

rule build-airfield {
  priority lerp(580, 680, air-weight)
  category economy exclusive
  do produce-airfield
  require air-weight > 0.1
  require not is-rushed()
  require not queue-busy(Building)
  require can-build-role(airfield)
  require not has-role(airfield)
  require power-excess >= 0
  require cash >= 500
}

rule build-service-depot {
  priority 570
  category economy exclusive
  do produce-service-depot
  require vehicle-weight > 0.3
  require not is-rushed()
  require not queue-busy(Building)
  require can-build-role(service-depot)
  require not has-role(service-depot)
  require has-role(war-factory)
  require power-excess >= 0
  require cash >= 1200
}

rule build-naval-yard {
  priority lerp(580, 680, naval-weight)
  category economy exclusive
  do produce-naval-yard
  require naval-weight > 0.1
  require not is-rushed()
  require map-has-water()
  require not queue-busy(Building)
  require can-build-role(naval-yard)
  require not has-role(naval-yard)
  require power-excess >= 0
  require cash >= 500
}

rule build-base-defense {
  priority lerp(400, 600, ground-defense-priority)
  category defense exclusive
  do produce-defense
  require ground-defense-priority > 0.2
  require not queue-busy(Defense)
  require power-excess >= 0
  require any-ground-defense-buildable()
  require ground-defense-count() < defense-cap()
  require affordable(lerp(1500, 300, ground-defense-priority))
}

rule build-base-defense-rush {
  priority lerp(400, 600, ground-defense-priority) + 100
  category defense exclusive
  because "under a rush, defenses go up before the next refinery"
  do produce-defense
  require ground-defense-priority > 0.2
  require is-rushed()
  require not queue-busy(Defense)
  require power-excess >= 0
  require any-ground-defense-buildable()
  require ground-defense-count() < defense-cap()
  require cash >= 200
}

rule build-aa-defense {
  priority lerp(400, 600, air-defense-priority)
  category defense exclusive
  do produce-aa-defense
  require air-defense-priority > 0.3
  require not queue-busy(Defense)
  require power-excess >= 0
  require can-build-role(aa-defense)
  require role-count(aa-defense) < lerp(2, 5, air-defense-priority)
  require affordable(lerp(1200, 500, air-defense-priority))
}

rule build-gap-generator {
  priority lerp(400, 550, ground-defense-priority)
  category defense exclusive
  do produce-gap-generator
  require ground-defense-priority > 0.3
  require tech-priority > 0.3
  require not is-rushed()
  require not queue-busy(Defense)
  require power-excess >= 0
  require can-build-role(gap-generator)
  require has-role(tech-center)
  require role-count(gap-generator) < lerp(1, 2, ground-defense-priority)
  require affordable(800)
}

rule build-tech-center {
  priority lerp(600, 660, tech-priority)
  category economy exclusive
  do produce-tech-center
  require tech-priority > 0.4
  require not is-rushed()
  require not queue-busy(Building)
  require can-build-role(tech-center)
  require not has-role(tech-center)
  require has-role(radar)
  require power-excess >= 0
  require cash >= 1500
}

rule build-missile-silo {
  priority 650
  category superweapon-build exclusive
  do produce-missile-silo
  require superweapon-priority > 0.3
  require not is-rushed()
  require not queue-busy(Defense)
  require can-build-role(missile-silo)
  require not has-role(missile-silo)
  require has-role(tech-center)
  require power-excess >= 0
  require cash >= 2500
}

rule build-iron-curtain {
  priority 640
  category superweapon-build exclusive
  do produce-iron-curtain
  require superweapon-priority > 0.3
  require not is-rushed()
  require not queue-busy(Defense)
  require can-build-role(iron-curtain)
  require not has-role(iron-curtain)
  require has-role(tech-center)
  require power-excess >= 0
  require cash >= 2500
}

rule build-extra-barracks {
  priority 500
  category economy exclusive
  do produce-barracks
  require infantry-weight > 0.6
  require not is-rushed()
  require not queue-busy(Building)
  require can-build-role(barracks)
  require role-count(barracks) < lerp(1, 3, infantry-weight)
  require power-excess >= 0
  require cash >= 300
}

rule build-extra-war-factory {
  priority 490
  category economy exclusive
  do produce-war-factory
  require vehicle-weight > 0.6
  require not is-rushed()
  require not queue-busy(Building)
  require can-build-role(war-factory)
  require role-count(war-factory) < lerp(1, 2, vehicle-weight)
  require power-excess >= 0
  require cash >= 2000
}

rule build-extra-airfield {
  priority 480
  category economy exclusive
  because "grow pads to match the doctrine's aircraft ambition, but only once the ones we have fill up"
  do produce-airfield
  require air-weight > 0.1
  require not is-rushed()
  require not queue-busy(Building)
  require can-build-role(airfield)
  require aircraft-capacity < lerp(2, 8, air-weight)
  require combat-aircraft-count >= aircraft-capacity - 1
  require power-excess >= 0
  require cash >= 500
}

rule build-extra-naval-yard {
  priority 470
  category economy exclusive
  do produce-naval-yard
  require naval-weight > 0.6
  require not is-rushed()
  require map-has-water()
  require not queue-busy(Building)
  require can-build-role(naval-yard)
  require role-count(naval-yard) < lerp(1, 2, naval-weight)
  require role-count(submarine) + role-count(destroyer) >= lerp(3, 8, naval-weight) - 1
  require power-excess >= 0
  require cash >= 500
}

rule build-flame-tower-for-flamethrower {
  priority 555
  category defense exclusive
  do produce-flame-tower
  require specialized-infantry-weight > 0.1
  require prefers-flamethrower > 0
  require not queue-busy(Defense)
  require can-build-role(flame-tower)
  require not has-role(flame-tower)
  require has-role(barracks)
  require power-excess >= 0
  require cash >= 600
}

rule build-tech-center-for-shock-trooper {
  priority 560
  category economy exclusive
  do produce-tech-center
  require specialized-infantry-weight > 0.1
  require prefers-shock-trooper > 0
  require tech-priority <= 0.4
  require not queue-busy(Building)
  require can-build-role(tech-center)
  require not has-role(tech-center)
  require has-role(radar)
  require power-excess >= 0
  require cash >= 1500
}

rule build-tesla-coil-for-shock-trooper {
  priority 555
  category defense exclusive
  do produce-tesla-coil
  require specialized-infantry-weight > 0.1
  require prefers-shock-trooper > 0
  require not queue-busy(Defense)
  require can-build-role(tesla-coil)
  require not has-role(tesla-coil)
  require has-role(barracks)
  require power-excess >= 0
  require cash >= 800
}

rule produce-specialist-infantry {
  priority specialist-base()
  category produce-infantry exclusive
  do produce-specialist-infantry
  require specialized-infantry-weight > 0.1
  require has-role(barracks)
  require not queue-busy(Infantry)
  require can-build-any-specialist()
  require specialist-infantry-count < lerp(1, 6, specialized-infantry-weight)
  require infantry-reserves(300)
}

rule produce-infantry {
  priority infantry-base()
  category produce-infantry exclusive
  do produce-infantry
  require infantry-weight > 0.1
  require not axis-burned(infantry)
  require has-role(barracks)
  require not queue-busy(Infantry)
  require can-build(Infantry, e1)
  require count(e1) < lerp(8, 20, infantry-weight)
  require infantry-reserves(100)
}

rule produce-infantry-rush {
  priority infantry-base() + 20
  category produce-infantry exclusive
  because "under a rush, cheap rifles have to spawn now rather than behind tech savings"
  do produce-infantry
  require infantry-weight > 0.1
  require is-rushed()
  require not axis-burned(infantry)
  require has-role(barracks)
  require not queue-busy(Infantry)
  require can-build(Infantry, e1)
  require count(e1) < lerp(8, 20, infantry-weight) * 2
  require cash >= 50
}

rule produce-bridge-infantry {
  priority infantry-base() - 5
  category produce-infantry exclusive
  because "extra rifles while the doctrine's production buildings are still missing"
  do produce-infantry
  require infantry-weight > 0.1
  require air-weight > 0.1 or naval-weight > 0.1 or vehicle-weight > 0.2
  require not axis-burned(infantry)
  require has-role(barracks)
  require not queue-busy(Infantry)
  require can-build(Infantry, e1)
  require (air-weight > 0.1 and not has-role(airfield))
       or (naval-weight > 0.1 and not has-role(naval-yard))
       or (vehicle-weight > 0.2 and not has-role(war-factory))
  require count(e1) >= lerp(8, 20, infantry-weight)
  require count(e1) < lerp(8, 20, infantry-weight)
        + select(air-weight > 0.1, lerp(2, 5, air-weight), 0.0)
        + select(naval-weight > 0.1, lerp(1, 4, naval-weight), 0.0)
        + select(vehicle-weight > 0.2, lerp(1, 3, vehicle-weight), 0.0)
  require infantry-reserves(100)
}

rule produce-grenadier {
  priority infantry-base() - 2
  category produce-infantry exclusive
  do produce-grenadier
  require infantry-weight > 0.2
  require not axis-burned(infantry)
  require has-role(barracks)
  require not queue-busy(Infantry)
  require can-build-role(grenadier)
  require role-count(grenadier) < lerp(2, 6, infantry-weight)
  require infantry-reserves(160)
}

rule build-kennel {
  priority 550
  category economy exclusive
  do produce-kennel
  require infantry-weight > 0.2
  require not queue-busy(Building)
  require can-build-role(kennel)
  require not has-role(kennel)
  require power-excess >= 0
  require cash >= 300
}

rule produce-attack-dog {
  priority infantry-base() + 10
  category produce-infantry exclusive
  because "the first dog out of the kennel is the scout, so it outranks rifles until the cap"
  do produce-attack-dog
  require infantry-weight > 0.2
  require has-role(kennel)
  require not queue-busy(Infantry)
  require can-build-role(attack-dog)
  require role-count(attack-dog) < lerp(1, 3, infantry-weight)
  require infantry-reserves(200)
}

rule produce-spy {
  priority 440
  category produce-infantry exclusive
  do produce-spy
  require capture-priority > 0.2 or scout-priority > 0.3
  require has-role(barracks)
  require has-role(radar)
  require not queue-busy(Infantry)
  require can-build-role(spy)
  require role-count(spy) < 1
  require infantry-reserves(500)
}

rule produce-vehicle {
  priority 480
  category produce-vehicle exclusive
  do produce-vehicle
  require vehicle-weight > 0.1
  require not axis-burned(vehicle)
  require has-role(war-factory)
  require not queue-busy(Vehicle)
  require can-build-any-combat-vehicle()
  require combat-vehicle-count < lerp(3, 10, vehicle-weight)
  require scaled-reserves(800)
}

rule produce-aircraft {
  priority 460
  category produce-aircraft exclusive
  do produce-aircraft
  require air-weight > 0.1
  require not axis-burned(air)
  require has-role(airfield)
  require not queue-busy(Aircraft)
  require can-build-any-combat-aircraft()
  require combat-aircraft-count < lerp(2, 8, air-weight)
  require combat-aircraft-count < aircraft-capacity
  require reserves(800)
}

rule produce-ship {
  priority 440
  category produce-ship exclusive
  do produce-ship
  require naval-weight > 0.1
  require not axis-burned(naval)
  require map-has-water()
  require has-role(naval-yard)
  require not queue-busy(Ship)
  require can-build-role(submarine) or can-build-role(destroyer)
  require role-count(submarine) + role-count(destroyer) < lerp(3, 8, naval-weight)
  require reserves(800)
}

rule produce-gunboat {
  priority 435
  category produce-ship exclusive
  do produce-gunboat
  require naval-weight > 0.1
  require map-has-water()
  require has-role(naval-yard)
  require not queue-busy(Ship)
  require can-build-role(gunboat)
  require role-count(gunboat) < lerp(1, 4, naval-weight)
  require reserves(500)
}

rule produce-rocket-soldier {
  priority 495
  category produce-infantry exclusive
  do produce-rocket-soldier
  require infantry-weight > 0.1
  require tech-priority > 0.3
  require not axis-burned(infantry)
  require has-role(barracks)
  require not queue-busy(Infantry)
  require can-build-role(rocket-soldier)
  require role-count(rocket-soldier) < lerp(2, 8, tech-priority * infantry-weight)
  require infantry-reserves(300)
}

rule produce-heavy-vehicle {
  priority 475
  category produce-vehicle exclusive
  do produce-heavy-vehicle
  require vehicle-weight > 0.1
  require tech-priority > 0.3
  require has-role(war-factory)
  require has-role(tech-center)
  require not queue-busy(Vehicle)
  require can-build-role(heavy-tank) or can-build-role(medium-tank)
  require role-count(heavy-tank) + role-count(medium-tank) < lerp(1, 5, tech-priority * vehicle-weight)
  require reserves(1200)
}

rule produce-scout-vehicle {
  priority 465
  category produce-vehicle exclusive
  do produce-scout-vehicle
  require vehicle-weight > 0.1
  require not has-enemy-intel()
  require has-role(war-factory)
  require not queue-busy(Vehicle)
  require can-build-role(ranger) or can-build-role(light-tank)
  require not has-role(ranger)
  require not has-scout()
  require reserves(500)
}

rule produce-siege-vehicle {
  priority trunc(select(prefers-v2-launcher > 0 or prefers-artillery > 0, 485.0, 460.0))
  category produce-vehicle exclusive
  because "when stand-off is the plan, siege has to beat generic vehicles to the queue"
  do produce-siege-vehicle
  require vehicle-weight > 0.2
  require has-role(war-factory)
  require has-role(radar)
  require not queue-busy(Vehicle)
  require can-build-role(artillery) or can-build-role(v2-launcher)
  require role-count(artillery) + role-count(v2-launcher)
        < select(siege-vehicle-first > 0, lerp(3, 8, vehicle-weight), lerp(1, 3, vehicle-weight))
  require scaled-reserves(900)
}

rule produce-flak-truck {
  priority 470
  category produce-vehicle exclusive
  do produce-flak-truck
  require air-defense-priority > 0.2
  require has-role(war-factory)
  require not queue-busy(Vehicle)
  require can-build-role(flak-truck)
  require role-count(flak-truck) < lerp(1, 3, air-defense-priority)
  require reserves(600)
}

rule produce-mad-tank {
  priority 455
  category produce-vehicle exclusive
  do produce-mad-tank
  require aggression > 0.3
  require tech-priority > 0.3
  require has-role(war-factory)
  require has-role(tech-center)
  require not queue-busy(Vehicle)
  require can-build-role(mad-tank)
  require role-count(mad-tank) < lerp(1, 2, aggression)
  require reserves(2000)
}

rule produce-minelayer {
  priority 450
  category produce-vehicle exclusive
  do produce-minelayer
  require ground-defense-priority > 0.3
  require has-role(war-factory)
  require has-role(service-depot)
  require not queue-busy(Vehicle)
  require can-build-role(minelayer)
  require role-count(minelayer) < lerp(1, 2, ground-defense-priority)
  require reserves(800)
}

rule lay-mines {
  priority 300
  category minelayer
  do lay-mines
  require count(idle-minelayers) > 0
}

rule produce-basic-aircraft {
  priority 445
  category produce-aircraft exclusive
  do produce-basic-aircraft
  require air-weight > 0.2
  require has-role(airfield)
  require not queue-busy(Aircraft)
  require can-build-role(basic-aircraft)
  require role-count(basic-aircraft) < lerp(1, 3, air-weight)
  require combat-aircraft-count < aircraft-capacity
  require reserves(1500)
}

rule produce-attack-aircraft {
  priority 455
  category produce-aircraft exclusive
  do produce-advanced-aircraft
  require air-weight > 0.1
  require tech-priority > 0.4
  require has-role(airfield)
  require not queue-busy(Aircraft)
  require can-build-role(advanced-aircraft)
  require role-count(advanced-aircraft) < lerp(1, 4, tech-priority * air-weight)
  require combat-aircraft-count < aircraft-capacity
  require reserves(1500)
}

rule produce-advanced-ship {
  priority trunc(select(tech-naval-first > 0, 445.0, 430.0))
  category produce-ship exclusive
  do produce-advanced-ship
  require naval-weight > 0.1
  require tech-priority > 0.3
  require map-has-water()
  require has-role(naval-yard)
  require not queue-busy(Ship)
  require can-build-role(cruiser) or can-build-role(destroyer) or can-build-role(missile-sub)
  require role-count(cruiser) + role-count(destroyer) + role-count(missile-sub)
        < select(tech-naval-first > 0, lerp(3, 8, naval-weight), lerp(1, 3, tech-priority * naval-weight))
  require reserves(2000)
}

rule form-defense-squad {
  priority defend-priority() + 5
  category squad-form
  because "reserve defenders only when there is also enough surplus to form an attack squad"
  do form-squad(ground-defense, Ground, lerp(2, 5, ground-defense-priority), Defend)
  require ground-defense-priority > 0.3
  require (not squad-exists(ground-defense)
           and count(unassigned-idle-ground) >= lerp(2, 5, ground-defense-priority))
       or (squad-needs-reinforcement(ground-defense) and count(unassigned-idle-ground) >= 1)
  require base-under-attack()
       or count(unassigned-idle-ground)
          >= lerp(2, 5, ground-defense-priority) + ground-form-threshold()
}

rule squad-defend-base {
  priority defend-priority()
  category combat
  do squad-defend(ground-defense)
  require ground-defense-priority > 0.3
  require squad-exists(ground-defense)
  require squad-idle-count(ground-defense) > 0
  require base-under-attack()
}

rule defend-base {
  priority defend-priority()
  category combat
  because "no reserved squad at low defense priority, so scramble whatever is idle"
  do defend-base
  require ground-defense-priority <= 0.3
  require base-under-attack()
  require count(idle-ground-units) >= lerp(3, 1, ground-defense-priority)
}

rule defend-base-air {
  priority lerp(350, 500, air-defense-priority)
  category air-combat
  do air-defend-base
  require base-under-attack()
  require count(idle-combat-aircraft) > 0
}

rule form-ground-attack {
  priority attack-priority() + 5
  category squad-form
  do form-squad(ground-attack, Ground, ground-attack-group-size, Attack)
  require (not squad-exists(ground-attack)
           and count(unassigned-idle-ground) >= ground-form-threshold())
       or (squad-needs-reinforcement(ground-attack) and count(unassigned-idle-ground) >= 1)
}

rule squad-attack {
  priority attack-priority()
  category ground-attack-choice exclusive
  do squad-attack-move(ground-attack)
  require squad-exists(ground-attack)
  require squad-ready-ratio(ground-attack) >= round2(activation())
  require exists best-ground-target or exists nearest-enemy
  require defense-floor-holds()
}

rule squad-reengage {
  priority attack-priority() - 2
  category combat
  because "catches stragglers finishing an order while the squad presses forward"
  do squad-attack-move(ground-attack)
  require squad-exists(ground-attack)
  require squad-idle-count(ground-attack) > 0
  require exists best-ground-target or exists nearest-enemy
}

rule squad-attack-known-base {
  priority trunc(select(aggression >= 0.3,
                        attack-priority() + 5,
                        attack-priority() - 10))
  category ground-attack-choice exclusive
  because "aggressive doctrines press the base and let base defenses handle raiders"
  do squad-attack-known-base(ground-attack, aggression)
  require squad-exists(ground-attack)
  require squad-ready-ratio(ground-attack) >= round2(activation())
  require has-enemy-intel()
  require defense-floor-holds()
}

rule form-air-attack {
  priority attack-priority() - 5 + 5
  category squad-form
  do form-squad(air-attack, Air, air-attack-group-size, Attack)
  require air-weight > 0.1
  require (not squad-exists(air-attack)
           and count(unassigned-idle-air) >= trunc(max(2.0, air-attack-group-size * 6 / 10)))
       or (squad-needs-reinforcement(air-attack) and count(unassigned-idle-air) >= 1)
}

rule squad-air-attack {
  priority attack-priority() - 5
  category air-combat
  do squad-air-strike(air-attack)
  require air-weight > 0.1
  require squad-exists(air-attack)
  require squad-ready-ratio(air-attack) >= round2(activation())
  require exists best-air-target
}

rule squad-air-reengage {
  priority attack-priority() - 5 - 2
  category air-combat
  do squad-air-strike(air-attack)
  require air-weight > 0.1
  require squad-exists(air-attack)
  require squad-idle-count(air-attack) > 0
  require exists best-air-target
}

rule squad-air-attack-known-base {
  priority attack-priority() - 5 - 10
  category air-combat
  do squad-attack-known-base(air-attack, aggression)
  require air-weight > 0.1
  require squad-exists(air-attack)
  require squad-ready-ratio(air-attack) >= round2(activation())
  require not enemies-visible
  require has-enemy-intel()
}

rule form-naval-attack {
  priority attack-priority() - 15 + 5
  category squad-form
  do form-squad(naval-attack, Naval, naval-attack-group-size, Attack)
  require naval-weight > 0.1
  require map-has-water()
  require (not squad-exists(naval-attack)
           and count(unassigned-idle-naval) >= trunc(max(2.0, naval-attack-group-size * 6 / 10)))
       or (squad-needs-reinforcement(naval-attack) and count(unassigned-idle-naval) >= 1)
}

rule squad-naval-attack {
  priority attack-priority() - 15
  category naval-combat
  do squad-attack-move(naval-attack)
  require naval-weight > 0.1
  require map-has-water()
  require squad-exists(naval-attack)
  require squad-ready-ratio(naval-attack) >= round2(activation())
  require exists nearest-enemy
}

rule squad-naval-reengage {
  priority attack-priority() - 15 - 2
  category naval-combat
  do squad-attack-move(naval-attack)
  require naval-weight > 0.1
  require map-has-water()
  require squad-exists(naval-attack)
  require squad-idle-count(naval-attack) > 0
  require exists nearest-enemy
}

rule squad-naval-attack-known-base {
  priority attack-priority() - 15 - 10
  category naval-combat
  do squad-attack-known-base(naval-attack, aggression)
  require naval-weight > 0.1
  require map-has-water()
  require squad-exists(naval-attack)
  require squad-ready-ratio(naval-attack) >= round2(activation())
  require not enemies-visible
  require has-enemy-intel()
}

rule fire-nuke {
  priority 880
  category superweapon exclusive
  do fire-nuke
  require superweapon-priority > 0.1
  require support-power-ready(NukePowerInfoOrder)
}

rule fire-iron-curtain {
  priority 870
  category superweapon exclusive
  do fire-iron-curtain
  require superweapon-priority > 0.1
  require support-power-ready(GrantExternalConditionPowerInfoOrder)
  require count(idle-ground-units) >= 3
}

rule fire-spy-plane {
  priority 860
  category superweapon exclusive
  do fire-spy-plane
  require air-weight > 0.1
  require support-power-ready(SovietSpyPlane)
  require not has-enemy-intel()
}

rule fire-spy-plane-update {
  priority 250
  category superweapon exclusive
  do fire-spy-plane
  require air-weight > 0.1
  require support-power-ready(SovietSpyPlane)
  require has-enemy-intel()
  require not enemies-visible
}

rule fire-paratroopers {
  priority 855
  category superweapon exclusive
  do fire-paratroopers
  require air-weight > 0.1
  require support-power-ready(SovietParatroopers)
  require has-enemy-intel() or enemies-visible
}

rule fire-parabombs {
  priority 845
  category superweapon exclusive
  do fire-parabombs
  require air-weight > 0.1
  require support-power-ready(UkraineParabombs)
  require has-enemy-intel() or enemies-visible
}

rule retreat-damaged-units {
  priority retreat-priority()
  category micro
  do retreat-damaged-units(retreat-threshold())
  require count(damaged-combat-units(round2(retreat-threshold()))) > 0
}

rule clear-healed-units {
  priority 500
  category micro
  because "runs every tick so healed units rejoin the combat pool promptly"
  do clear-healed-units(retreat-threshold())
  require has-retreating-units()
}

rule recall-overextended-ground-attack {
  priority retreat-priority() - 10
  category micro
  because "aggressive doctrines let units roam further before the leash pulls"
  do recall-overextended(ground-attack, leash())
  require squad-exists(ground-attack)
  require count(overextended-squad-members(ground-attack, round2(leash()))) > 0
}

rule recall-overextended-naval-attack {
  priority retreat-priority() - 10
  category micro
  do recall-overextended(naval-attack, leash())
  require squad-exists(naval-attack)
  require count(overextended-squad-members(naval-attack, round2(leash()))) > 0
}

rule squad-disengage-ground-attack {
  priority retreat-priority() - 5
  category micro
  because "pure aggression never disengages, so this is absent at aggression 1"
  do squad-disengage(ground-attack)
  require aggression < 1.0
  require squad-exists(ground-attack)
  require squad-away-from-base(ground-attack, 0.1)
  require squad-threat-ratio(ground-attack, 0.1) > round2(lerpf(1.5, 3.0, aggression))
}

rule squad-disengage-naval-attack {
  priority retreat-priority() - 5
  category micro
  do squad-disengage(naval-attack)
  require aggression < 1.0
  require squad-exists(naval-attack)
  require squad-away-from-base(naval-attack, 0.1)
  require squad-threat-ratio(naval-attack, 0.1) > round2(lerpf(1.5, 3.0, aggression))
}

rule squad-focus-fire {
  priority lerp(200, 400, aggression) + 1
  category micro
  do squad-focus-fire(ground-attack)
  require aggression > 0.2
  require squad-exists(ground-attack)
  require squad-ready-ratio(ground-attack) >= round2(activation())
  require exists best-ground-target
}

rule flee-harvesters {
  priority lerp(150, 300, economy-priority)
  category micro
  do flee-harvesters(lerpf(0.05, 0.15, economy-priority))
  require economy-priority > 0.1
  require enemies-visible
  require count(harvesters-in-danger(round2(lerpf(0.05, 0.15, economy-priority)))) > 0
}

rule scout-with-scouts {
  priority lerp(250, 400, scout-priority) + 5
  category recon
  because "rangers keep patrolling so intel stays fresh rather than going stale"
  do scout-patrol
  require count(idle-scouts) > 0
}

rule scout-with-idle-units {
  priority lerp(250, 400, scout-priority)
  category recon
  do scout
  require not enemies-visible
  require count(unassigned-idle-ground) >= ground-attack-group-size
}
