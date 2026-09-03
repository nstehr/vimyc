rule build-barracks {
  priority 700
  category ecomomy exclusive
  do       produce-baracks

  require not queue-busy(Bulding)
  require can-build-role(war-facotry)
  require has-building(e1)
  require nearest-enemy
  require count(powr) and cash
}
