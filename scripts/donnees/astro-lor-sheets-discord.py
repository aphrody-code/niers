"""Lecture unique du message autorisé avec la configuration existante du VPS."""

# /// script
# requires-python = ">=3.10"
# dependencies = ["Pillow>=12", "python-dotenv>=1"]
# ///

import hashlib
import io
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import urlsplit
from urllib.request import Request, urlopen

from dotenv import dotenv_values
from PIL import Image

SOURCE = 'https://discord.com/channels/1544475258591907961/1544482971934007336/1545590117895250101'

# Le dossier de sortie est OBLIGATOIRE et n'a pas de valeur par défaut : ce script nomme ce
# qu'il récupère `attachment-<id Discord>-<empreinte>.jpg`, pas `01-og-tenue-jaune.jpg`. Seul
# un regard humain sait ce que montre une planche, donc le renommage vers
# `data/oc/astro-lor/source/` reste un geste manuel. Avec un défaut pointant dessus, un simple
# rejeu y déverserait douze doublons sous leur nom brut — c'est exactement ce qui était arrivé.
if len(sys.argv) != 2:
    raise SystemExit(
        'usage : uv run scripts/donnees/astro-lor-sheets-discord.py <dossier de sortie>\n'
        '        (un dossier de travail, PAS data/oc/astro-lor/source —\n'
        '         cf. data/oc/astro-lor/README.md)'
    )
OUTPUT = Path(sys.argv[1])
ENV_KEYS = ('WONDERBOT_DISCORD_TOKEN', 'DISCORD_BOT_TOKEN', 'DISCORD_TOKEN')
configured_env = os.environ.get('NIERS_WONDERBOT_ENV')
env_candidates = [
    Path(configured_env) if configured_env else None,
    Path.home() / '.config' / 'rgfr' / 'wonderbot.env',
    Path('/home/ubuntu/.config/niers/wonderbot.env'),
]
env_path = next((path for path in env_candidates if path and path.is_file()), None)
config = dotenv_values(env_path, interpolate=False) if env_path else {}
token = next((os.environ.get(k) or config.get(k) for k in ENV_KEYS
              if os.environ.get(k) or config.get(k)), None)
if not token or token.startswith(('$', 'eyJ2Ijo')):
    raise SystemExit('CONFIG_BOT_VPS_ABSENTE_OU_INUTILISABLE')

def api(path):
    request = Request('https://discord.com/api/v10/' + path, headers={'Authorization': 'Bot ' + token, 'User-Agent': 'Wonderbot-ImageExport/1.0'})
    try:
        with urlopen(request, timeout=25) as response:
            result = json.load(response)
            print('API_HTTP', response.status, path, flush=True)
            return result
    except HTTPError as error:
        try:
            body = json.loads(error.read(4096))
        except Exception:
            body = {}
        message = str(body.get('message', '')).replace(token, '[REDACTED]')
        print(json.dumps({'http': error.code, 'discord_code': body.get('code'), 'message': message[:200] if 'http' not in message else '[URL omitted]'}), flush=True)
        raise SystemExit(2)
    except Exception as error:
        print('NETWORK_ERROR', type(error).__name__, flush=True)
        raise SystemExit(3)

identity = api('users/@me')
if identity.get('bot') is not True:
    raise SystemExit('IDENTITE_NON_BOT_ARRET')
print('BOT_SERVICE_NIERS_WONDERBOT', identity['id'], flush=True)
message = api('channels/1544482971934007336/messages/1545590117895250101')
print('MESSAGE_STRUCTURE', json.dumps({'type': message.get('type'), 'keys': list(message), 'attachments': len(message.get('attachments', [])), 'embeds': len(message.get('embeds', [])), 'snapshots': len(message.get('message_snapshots', [])), 'components': len(message.get('components', [])), 'content_present': bool(message.get('content')), 'reference_type': (message.get('message_reference') or {}).get('type')}), flush=True)
assets = []
payloads = [message] + [s['message'] for s in message.get('message_snapshots', []) if isinstance(s.get('message'), dict)]
for attachment in [a for p in payloads for a in p.get('attachments', [])]:
    if str(attachment.get('content_type', '')).startswith('image/') or attachment.get('width'):
        assets.append(('attachment', attachment['id'], attachment['url']))
for index, embed in enumerate([e for p in payloads for e in p.get('embeds', [])]):
    for key in ('image', 'thumbnail'):
        media = embed.get(key) or {}
        if media.get('url'):
            assets.append(('embed_' + key, str(index), media.get('proxy_url') or media['url']))

manifest = {'source_message': SOURCE, 'bot_id': identity['id'], 'service': 'niers-wonderbot.service', 'message_http_status': 200, 'retrieved_at': datetime.now(timezone.utc).isoformat(), 'images': [], 'skipped': [], 'linked_embed_hosts': sorted({urlsplit(e['url']).hostname for e in message.get('embeds', []) if e.get('url') and urlsplit(e['url']).hostname})}
OUTPUT.mkdir(exist_ok=True)
seen = set()
for kind, source_id, url in assets:
    if url in seen:
        continue
    seen.add(url)
    parsed = urlsplit(url)
    if parsed.scheme != 'https' or parsed.hostname not in ('cdn.discordapp.com', 'media.discordapp.net'):
        manifest['skipped'].append({'kind': kind, 'id': source_id, 'reason': 'external_host_not_downloaded'})
        continue
    try:
        with urlopen(Request(url, headers={'User-Agent': 'Wonderbot-ImageExport/1.0'}), timeout=40) as response:
            data = response.read(100 * 1024 * 1024 + 1)
            status = response.status
        if len(data) > 100 * 1024 * 1024:
            raise ValueError('image too large')
        with Image.open(io.BytesIO(data)) as image:
            image.verify()
        with Image.open(io.BytesIO(data)) as image:
            image.load()
            width, height = image.size
            image_format = image.format
        digest = hashlib.sha256(data).hexdigest()
        extension = {'JPEG': 'jpg', 'PNG': 'png', 'WEBP': 'webp', 'GIF': 'gif', 'AVIF': 'avif'}.get(image_format, image_format.lower())
        name = f'{kind}-{source_id}-{digest[:16]}.{extension}'
        target = OUTPUT / name
        if target.exists():
            if hashlib.sha256(target.read_bytes()).hexdigest() != digest:
                raise ValueError('existing file differs')
        else:
            with target.open('xb') as stream:
                stream.write(data)
        manifest['images'].append({'file': name, 'kind': kind, 'source_id': source_id, 'http_status': status, 'width': width, 'height': height, 'format': image_format, 'bytes': len(data), 'sha256': digest})
    except Exception as error:
        manifest['skipped'].append({'kind': kind, 'id': source_id, 'reason': type(error).__name__})
manifest_path = OUTPUT / ('manifest-1545590117895250101-' + datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S%f') + '.json')
with manifest_path.open('x', encoding='utf-8') as stream:
    json.dump(manifest, stream, ensure_ascii=False, indent=2)
print(json.dumps(manifest, ensure_ascii=False), flush=True)
print('MANIFEST', str(manifest_path), flush=True)
