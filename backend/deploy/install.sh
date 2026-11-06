#!/usr/bin/env bash
# Ставит бекенд лаунчера на Ubuntu 24 рядом с сайтом из /var/www/html.
# Повторный запуск безопасен: обновляет код и перезапускает контейнер.
set -euo pipefail

REPO_URL="https://github.com/BloodRayT/launcher.git"
INSTALL_DIR="/opt/fenixcraft"
WEB_ROOT="/var/www/html"
SNIPPET="/etc/nginx/snippets/fenixcraft-launcher.conf"
HEALTH_URL="http://127.0.0.1:8000/api/launcher/health"
PROFILES=("1.21.1-neoforge" "1.21.1-iris-neoforge")
PROFILE_DIRS=(mods shaderpacks resourcepacks config)

log() { printf '\n== %s\n' "$*"; }
fail() { printf '\nОшибка: %s\n' "$*" >&2; exit 1; }

[ "$(id -u)" -eq 0 ] || fail "запустите через sudo"

log "Установка пакетов"
apt-get update -q
apt-get install -y -q docker.io docker-compose-v2 git curl nginx
systemctl enable --now docker

log "Код бекенда в ${INSTALL_DIR}"
if [ -d "${INSTALL_DIR}/.git" ]; then
  git -C "${INSTALL_DIR}" pull --ff-only
else
  git clone --depth 1 "${REPO_URL}" "${INSTALL_DIR}"
fi
cd "${INSTALL_DIR}/backend"
if [ ! -f .env ]; then
  cp .env.example .env
fi
chmod 600 .env
for profile in "${PROFILES[@]}"; do
  for sub in "${PROFILE_DIRS[@]}"; do
    mkdir -p "storage/${profile}/${sub}"
  done
done

log "Запуск контейнера"
docker compose up -d --build
for _ in $(seq 1 30); do
  curl -fsS "${HEALTH_URL}" >/dev/null 2>&1 && break
  sleep 1
done
curl -fsS "${HEALTH_URL}" >/dev/null || fail "бекенд не ответил, смотрите: docker compose logs"

log "nginx: адрес /api/launcher/"
cat > "${SNIPPET}" <<'EOF'
location /api/launcher/ {
    proxy_pass http://127.0.0.1:8000;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_read_timeout 300s;
    # Моды отдаются потоком, без временных файлов nginx.
    proxy_max_temp_file_size 0;
}
EOF

site="$(grep -lE "^\s*root\s+${WEB_ROOT}/?;" /etc/nginx/sites-enabled/* 2>/dev/null | head -n 1 || true)"
[ -n "${site}" ] || fail "не найден сайт nginx с root ${WEB_ROOT}"
if ! grep -q "fenixcraft-launcher.conf" "${site}"; then
  backup="/root/nginx-$(basename "${site}").$(date +%Y%m%d%H%M%S).bak"
  cp -L "${site}" "${backup}"
  # Подключаем адрес бекенда в каждый server-блок сайта (строки с # не трогаются).
  sed -i --follow-symlinks '/^\s*server_name\s.*;/a\    include snippets/fenixcraft-launcher.conf;' "${site}"
  if ! nginx -t; then
    cp "${backup}" "$(readlink -f "${site}")"
    fail "nginx не принял настройки, файл ${site} восстановлен"
  fi
fi
systemctl reload nginx

if ufw status 2>/dev/null | grep -q "Status: active"; then
  ufw allow 80/tcp
fi

log "Проверка через nginx"
curl -fsS -H "Host: fenixcraft.ru" "http://127.0.0.1/api/launcher/health"
printf '\n\nГотово. Моды кладите в %s/backend/storage/1.21.1-iris-neoforge/mods\n' "${INSTALL_DIR}"
