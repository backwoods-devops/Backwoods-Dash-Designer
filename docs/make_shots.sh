#!/bin/sh
# Regenerates the manual's pictures: ./docs/make_shots.sh  (needs xvfb and ImageMagick). Output: docs/raw/<scene>.png
cd "$(dirname "$0")/.." || exit 1
mkdir -p docs/raw
for s in start draw_new draw trace wiz_start wiz_size wiz_wind wiz_front wiz_sections wiz_finish main main_design plan height walls shapes lighting export size wallview shape_ed shape_combo shape_3d leave cut story; do
  d=$(mktemp -d)
  DASHFW_DOCSHOT=$s LIBGL_ALWAYS_SOFTWARE=1 DASHFW_DIR=$d DASHFW_MSAA=0 xvfb-run -a -s "-screen 0 1480x900x24" sh -c "${DASHFW_BIN:-./target/release/bw-dash-designer} & sleep 18; import -window root docs/raw/$s.png; kill %1" >/dev/null 2>&1
  rm -rf "$d"
done
