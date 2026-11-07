#!/usr/bin/env bash
# Ставит бекенд лаунчера на Ubuntu рядом с сайтом из /var/www/html.
# Адрес /api/launcher/ подключается к тому веб-серверу, который уже отдаёт сайт: Apache или nginx.
# Повторный запуск безопасен: обновляет код и перезапускает контейнер.
set -euo pipefail

REPO_URL="https://github.com/BloodRayT/launcher.git"
INSTALL_DIR="/opt/fenixcraft"
WEB_ROOT="/var/www/html"
BACKEND="http://127.0.0.1:8000"
HEALTH_PATH="/api/launcher/health"
PROFILES=("1.21.1-neoforge" "1.21.1-iris-neoforge" "26.2-iris-neoforge")
PROFILE_DIRS=(mods shaderpacks resourcepacks config)
APACHE_CONF="fenixcraft-launcher"
NGINX_SNIPPET="/etc/nginx/snippets/fenixcraft-launcher.conf"

log() { printf '\n== %s\n' "$*"; }
fail() { printf '\nОшибка: %s\n' "$*" >&2; exit 1; }

port80_owner() {
  ss -ltnpH 'sport = :80' 2>/dev/null | grep -oE '"(apache2|nginx)"' | head -n 1 | tr -d '"' || true
}

detect_web_server() {
  local owner
  owner="$(port80_owner)"
  if [ -n "${owner}" ]; then
    echo "${owner}"
  elif command -v apache2ctl >/dev/null 2>&1; then
    echo "apache2"
  else
    echo "nginx"
  fi
}

install_backend() {
  log "Установка Docker"
  apt-get update -q
  apt-get install -y -q docker.io docker-compose-v2 git curl
  systemctl enable --now docker

  log "Код бекенда в ${INSTALL_DIR}"
  if [ -d "${INSTALL_DIR}/.git" ]; then
    git -C "${INSTALL_DIR}" pull --ff-only
  else
    git clone --depth 1 "${REPO_URL}" "${INSTALL_DIR}"
  fi
  cd "${INSTALL_DIR}/backend"
  [ -f .env ] || cp .env.example .env
  chmod 600 .env
  for profile in "${PROFILES[@]}"; do
    for sub in "${PROFILE_DIRS[@]}"; do
      mkdir -p "storage/${profile}/${sub}"
    done
  done

  # Скины и плащи для окна «Выбрать скин» и выбор игроков.
  mkdir -p catalog/skins catalog/capes data

  log "Запуск контейнера"
  docker compose up -d --build
  for _ in $(seq 1 30); do
    curl -fsS "${BACKEND}${HEALTH_PATH}" >/dev/null 2>&1 && return 0
    sleep 1
  done
  fail "бекенд не ответил, смотрите: cd ${INSTALL_DIR}/backend && docker compose logs"
}

setup_apache() {
  log "Apache: адрес /api/launcher/"
  a2enmod -q proxy proxy_http
  cat > "/etc/apache2/conf-available/${APACHE_CONF}.conf" <<EOF
# Бекенд лаунчера FenixCraft (backend/deploy/install.sh)
ProxyPass "/api/launcher/" "${BACKEND}/api/launcher/"
ProxyPassReverse "/api/launcher/" "${BACKEND}/api/launcher/"
ProxyTimeout 300
EOF
  a2enconf -q "${APACHE_CONF}"
  if ! apache2ctl configtest; then
    a2disconf -q "${APACHE_CONF}"
    fail "Apache не принял настройки, подключение отменено"
  fi
  systemctl restart apache2
}

setup_nginx() {
  log "nginx: адрес /api/launcher/"
  command -v nginx >/dev/null 2>&1 || apt-get install -y -q nginx
  cat > "${NGINX_SNIPPET}" <<EOF
location /api/launcher/ {
    proxy_pass ${BACKEND};
    proxy_set_header Host \$host;
    proxy_set_header X-Real-IP \$remote_addr;
    proxy_read_timeout 300s;
    # Моды отдаются потоком, без временных файлов nginx.
    proxy_max_temp_file_size 0;
}
EOF
  local site backup
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
  systemctl enable --now nginx
  systemctl reload nginx
}

main() {
  [ "$(id -u)" -eq 0 ] || fail "запустите через sudo"
  install_backend

  local web_server
  web_server="$(detect_web_server)"
  case "${web_server}" in
    apache2) setup_apache ;;
    nginx) setup_nginx ;;
  esac

  if ufw status 2>/dev/null | grep -q "Status: active"; then
    ufw allow 80/tcp
  fi

  log "Проверка через ${web_server}"
  curl -fsS -H "Host: fenixcraft.ru" "http://127.0.0.1${HEALTH_PATH}" \
    || fail "${web_server} не передаёт запросы бекенду"
  printf '\n\nГотово. Моды кладите в папки сборок:\n'
  for profile in "${PROFILES[@]}"; do
    printf '  %s/backend/storage/%s/mods\n' "${INSTALL_DIR}" "${profile}"
  done
  printf '\nСкины и плащи (PNG) для окна «Выбрать скин»:\n  %s/backend/catalog/skins\n  %s/backend/catalog/capes\n' \
    "${INSTALL_DIR}" "${INSTALL_DIR}"
  printf 'Мод CustomSkinLoader для сборки FenixCraft положите в %s/backend/storage/26.2-iris-neoforge/mods\n' \
    "${INSTALL_DIR}"
}

main "$@"
