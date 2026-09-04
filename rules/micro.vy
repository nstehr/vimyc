param aggression: float
param economy-priority: float
param scout-priority: float
param commit-ratio: float
param ground-attack-group-size: int

def activation() =
  select(commit-ratio > 0.0, commit-ratio, lerpf(0.6, 1.0, 1.0 - aggression))

def retreat-threshold() = lerpf(0.5, 0.15, aggression)
def retreat-priority() = lerp(380, 450, 1.0 - aggression)
def leash() = lerpf(0.25, 0.5, aggression)

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
