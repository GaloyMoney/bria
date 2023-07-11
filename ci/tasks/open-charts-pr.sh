#!/bin/bash

set -eu

export digest=$(cat ./latest-image/digest)

pushd charts-repo

ref=$(yq e '.bria.image.git_ref' charts/${CHARTS_SUBDIR}/values.yaml)
git checkout ${BRANCH}
old_ref=$(yq e '.bria.image.git_ref' charts/${CHARTS_SUBDIR}/values.yaml)

cat <<EOF >> ../body.md
# Bump ${CHARTS_SUBDIR} image

The ${CHARTS_SUBDIR} image will be bumped to digest:
\`\`\`
${digest}
\`\`\`

Code diff contained in this image:

https://github.com/GaloyMoney/bria/compare/${old_ref}...${ref}
EOF

export GH_TOKEN="$(ghtoken -b "${GH_APP_PRIVATE_KEY}" -i "${GH_APP_ID}" | jq -r '.token')"

gh pr close ${BOT_BRANCH} || true
gh pr create \
  --title "chore(deps) bump-bria-image-${ref}" \
  --body-file ../body.md \
  --base ${BRANCH} \
  --head ${BOT_BRANCH} \
  --label galoybot
