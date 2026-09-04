param infantry-weight: float
param vehicle-weight: float
param air-weight: float
param naval-weight: float
param tech-priority: float
param aggression: float
param ground-defense-priority: float
param air-defense-priority: float
param superweapon-priority: float
param specialized-infantry-weight: float
param capture-priority: float
param scout-priority: float

param prefers-flamethrower: int
param prefers-shock-trooper: int
param prefers-v2-launcher: int
param prefers-artillery: int
param specialist-infantry-first: int
param siege-vehicle-first: int
param tech-naval-first: int

def infantry-base() = trunc(select(specialist-infantry-first > 0, 490.0, 500.0))
def specialist-base() = trunc(select(specialist-infantry-first > 0, 500.0, 490.0))

def reserves(cost: int) =
  cash >= cost
  and (vehicle-weight <= 0.1 or has-role(radar) or cash >= cost + 1000)
  and (vehicle-weight <= 0.2 or has-role(war-factory) or not has-role(radar) or cash >= cost + 2000)
  and (tech-priority <= 0.4 or has-role(tech-center) or not has-role(radar) or cash >= cost + 1500)
  and (superweapon-priority <= 0.4 or has-role(missile-silo) or has-role(iron-curtain)
       or not has-role(tech-center) or cash >= cost + 2500)

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
