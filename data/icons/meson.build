# SPDX-FileCopyrightText: 2022  Emmanuele Bassi
# SPDX-License-Identifier: GPL-3.0-or-later

scalable_dir = 'hicolor/scalable/apps'
install_data(
  scalable_dir / ('@0@.svg').format(application_id),
  install_dir: get_option('datadir') / 'icons' / scalable_dir,
)

# PNG copies of the scalable icon, for launchers, docks and toolkits that cannot draw SVG
foreach size : ['16', '24', '32', '48', '64', '128', '256', '512']
  size_dir = 'hicolor/@0@x@0@/apps'.format(size)
  install_data(
    size_dir / 'io.github.k_s_harish_balagi.Ryuon.png',
    rename: '@0@.png'.format(application_id),
    install_dir: get_option('datadir') / 'icons' / size_dir,
  )
endforeach

symbolic_dir = 'hicolor/symbolic/apps'
install_data(
  symbolic_dir / ('@0@-symbolic.svg').format(application_id),
  install_dir: get_option('datadir') / 'icons' / symbolic_dir,
)
