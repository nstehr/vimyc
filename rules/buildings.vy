param vehicle-weight: float
param air-weight: float
param naval-weight: float
param infantry-weight: float
param tech-priority: float
param ground-defense-priority: float
param air-defense-priority: float
param superweapon-priority: float
param transport-assault: float

param prefers-radar-gated-primary: int
param prefers-v2-launcher: int
param prefers-artillery: int

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
