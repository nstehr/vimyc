param aggression: float
param naval-weight: float
param ground-attack-group-size: int

rule form-naval-squad {
  priority lerp(200, 400, aggression)
  category squad-form
  because "a doctrine that ignores the sea should not build a navy"
  require naval-weight >= 0.3
  require map-has-water()
  require count(unassigned-idle-naval) >= ground-attack-group-size
  do form-squad(naval-attack, Naval, ground-attack-group-size, Attack)
}

rule naval-attack-move {
  priority lerp(200, 400, aggression) - 10
  category combat
  require naval-weight >= 0.3
  require squad-exists(naval-attack)
  do squad-attack-move(naval-attack)
}
