"""Exercise migration and native settings through the shared config owner."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

binary = str(Path(sys.argv[1]).resolve())
base = Path.home()/'construction_side/demio/checks'
base.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(dir=base) as temporary:
    root = Path(temporary)
    config = root/'config.toml'
    legacy = root/'speaker-notes.json'
    env = dict(os.environ, DEMIO_CONFIG=str(config), DEMIO_HOME=str(root))
    def call(*arguments, ok=True):
        result = subprocess.run([binary, *arguments], env=env, text=True, capture_output=True, timeout=10)
        assert (result.returncode == 0) == ok, (result.stdout, result.stderr)
        return result
    def settings(*arguments):
        return json.loads(call('--overlay-settings', *arguments).stdout)
    original = '# Keep user configuration\ntheme = "ocean"\ncustom = "preserved"\n\n[channels.existing]\nname = "Existing scenario"\nmanifest = "/missing/scenario.json"\n'
    config.write_text(original)
    legacy.write_text('{"font_size":30,"alignment":"right"}')
    # Any normal CLI entry point migrates once, not only the presenter window.
    call('list')
    assert not legacy.exists() and config.read_text().startswith(original)
    assert settings() == {'font_size':30.0,'alignment':'right'}
    settings('26', 'center')
    assert config.read_text().startswith(original)
    assert settings() == {'font_size':26.0,'alignment':'center'}
    assert not legacy.exists()
    call('theme','forest')
    assert settings() == {'font_size':26.0,'alignment':'center'}
    assert json.loads(call('list','--json').stdout)[0]['id']=='existing'
    # A newer common section wins even if an old file reappears.
    legacy.write_text('{"font_size":44,"alignment":"left"}')
    assert settings() == {'font_size':26.0,'alignment':'center'}
    assert not legacy.exists()
    config.write_text('theme = "calm"\n[overlay]\nfont_size = 99\nalignment = "invalid"\n')
    assert settings() == {'font_size':48.0,'alignment':'left'}
    config.write_text('theme = "calm"\n[overlay]\nfont_size = "invalid"\n')
    assert settings() == {'font_size':22.0,'alignment':'left'}
    # A broken common config must never be replaced or consume old preferences.
    config.write_text('not valid TOML!')
    legacy.write_text('{"font_size":28,"alignment":"right"}')
    call('--overlay-settings',ok=False)
    assert config.read_text()=='not valid TOML!' and legacy.exists()
    config.unlink()
    assert settings() == {'font_size':28.0,'alignment':'right'}
    assert config.exists() and not legacy.exists()
    legacy.write_text('{broken json')
    settings()
    assert legacy.read_text()=='{broken json'
print('Shared config: migration, preservation, preference updates, CLI edits, defaults and damaged-file recovery passed.')
