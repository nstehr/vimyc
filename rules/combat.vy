param aggression: float
param ground-defense-priority: float
param air-defense-priority: float
param air-weight: float
param naval-weight: float
param superweapon-priority: float
param commit-ratio: float
param base-defense-floor: int
param ground-attack-group-size: int
param air-attack-group-size: int
param naval-attack-group-size: int

def activation() =
  select(commit-ratio > 0.0, commit-ratio, lerpf(0.6, 1.0, 1.0 - aggression))

def attack-priority() = lerp(200, 400, aggression)

def defend-priority() = lerp(350, 500, ground-defense-priority)

def ground-form-threshold() = trunc(max(3.0, ground-attack-group-size * 6 / 10))

def defense-floor-holds() =
  base-defense-floor <= 0
  or role-count(pillbox) + role-count(camo-pillbox) + role-count(turret)
     + role-count(flame-tower) + role-count(tesla-coil) >= base-defense-floor

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
