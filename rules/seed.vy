rule deploy-mcv {
  priority 1000
  category setup exclusive
  do       deploy-mcv

  require has-unit(mcv)
  require not has-building(fact)
}

rule place-ready-building {
  priority 900
  category economy exclusive
  do       place-building

  require queue-ready(Building)
}

rule build-power {
  priority 800
  category economy exclusive
  do       produce-power-plant

  require not queue-busy(Building)
  require can-build(Building, powr)
  require power-excess < 100 or count(powr) == 0
  require cash >= 300
}

rule build-refinery {
  priority 750
  category economy exclusive
  do       produce-refinery

  require not queue-busy(Building)
  require can-build(Building, proc)
  require not has-building(proc)
  require cash >= 2000
}

rule build-barracks {
  priority 700
  category economy exclusive
  do       produce-barracks

  require not queue-busy(Building)
  require can-build-role(barracks)
  require not has-role(barracks)
  require power-excess >= 0
  require cash >= 300
}

rule build-war-factory {
  priority 650
  category economy exclusive
  do       produce-war-factory

  require not queue-busy(Building)
  require can-build(Building, weap)
  require not has-building(weap)
  require power-excess >= 0
  require cash >= 2000
}

rule produce-infantry {
  priority 500
  category production
  do       produce-infantry

  require has-role(barracks)
  require not queue-busy(Infantry)
  require count(e1) < 10
  require cash >= 100
}

rule defend-base {
  priority 400
  category combat
  do       defend-base

  require base-under-attack
  require count(idle-ground-units) >= 2
}

rule scout-with-idle-units {
  priority 350
  category recon
  do       scout

  require not enemies-visible
  require not has-enemy-intel
  require count(idle-ground-units) >= 5
}

rule attack-idle-units {
  priority 300
  category combat
  do       attack-move-ground

  require count(idle-ground-units) >= 5
  require exists nearest-enemy
}

rule attack-known-base {
  priority 290
  category combat
  do       attack-known-base-ground

  require not enemies-visible
  require has-enemy-intel
  require count(idle-ground-units) >= 5
}

rule repair-buildings {
  priority 200
  category maintenance
  do       repair-buildings

  require count(damaged-buildings) > 0
}

rule return-idle-harvesters {
  priority 100
  category harvester
  do       send-harvesters

  require count(idle-harvesters) > 0
}
