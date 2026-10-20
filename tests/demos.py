"""Verify demo ownership, scenario inheritance, registration and real adapter inputs."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

binary = str(Path(sys.argv[1]).resolve())
base = Path.home()/'construction_side/demio/checks'
base.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='demo ownership ', dir=base) as temporary:
    root = Path(temporary)
    env = dict(os.environ, DEMIO_CONFIG=str(root/'config.toml'), DEMIO_RUNTIME_DIR=str(root/'runtime'), XDG_STATE_HOME=str(root/'state'))
    def cli(*args, ok=True):
        result = subprocess.run([binary,*map(str,args)], env=env, capture_output=True, text=True, timeout=15)
        assert (result.returncode==0)==ok, (args,result.stdout,result.stderr)
        return result
    def write(path, data):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data))
    for name in ['alpha', 'beta']:
        demo = root/name
        (demo/'shared').mkdir(parents=True)
        adapter = demo/'shared/adapter.py'
        adapter.write_text('''import json,os,sys
from pathlib import Path
for line in sys.stdin:
 r=json.loads(line)
 with Path("adapter.log").open("a") as out: out.write(json.dumps({"op":r["op"],"cwd":str(Path.cwd()),"scenario":os.environ["DEMIO_SCENARIO"],"parameters":json.loads(os.environ["DEMIO_PARAMETERS"])})+"\\n")
 print(json.dumps({"id":r["id"],"event":"done","ok":True}),flush=True)
 if r["op"]=="cleanup": break
''')
        manifest = {'schema_version':1,'id':name,'name':name.title()+' demo','project':'Example','description':'Shared environment','workspace':'.','adapter':[sys.executable,'-u',str(adapter)],'parameters':{'route':'/shared','text':'Default'},'scenarios':['scenarios/one/scenario.json','scenarios/two/scenario.json']}
        write(demo/'demo.json', manifest)
        for variant in ['one','two']:
            write(demo/f'scenarios/{variant}/scenario.json', {'schema_version':1,'id':name+'-'+variant,'demo':'../../demo.json','name':variant,'description':variant+' narration','parameters':{'route':'/'+variant},'steps':[{'id':'show','title':'Show the route','role':'Presenter','say':variant+' text'}]})
        cli('validate', demo/'demo.json')
        cli('register', demo/'demo.json')
        listing=json.loads(cli('list','--json').stdout)
        assert all(item['demo']['id']==name and item['demo']['folder']==str(demo) for item in listing if item['id'].startswith(name))
        for variant in ['one','two']:
            cli('doctor',name+'-'+variant)
            cli('run',name+'-'+variant,'--auto')
        actions=[json.loads(line) for line in (demo/'adapter.log').read_text().splitlines()]
        assert {row['parameters']['route'] for row in actions}=={'/one','/two'}
        assert all(row['parameters']['text']=='Default' and row['cwd']==str(demo) for row in actions)
        assert {row['scenario'] for row in actions}=={str(demo/'scenarios/one/scenario.json'),str(demo/'scenarios/two/scenario.json')}
    config_before=(root/'config.toml').read_bytes()
    first=root/'alpha/scenarios/one/scenario.json'
    first_created=json.loads(first.read_text())['created_at']
    alias=root/'alpha/old.channel.json'
    alias.symlink_to(first)
    config_path=root/'config.toml'
    saved_config=config_path.read_text()
    config_path.write_text(saved_config.replace(str(first),str(alias)))
    cli('doctor','alpha-one')
    config_path.write_text(saved_config)
    cli('register',root/'alpha/demo.json')
    assert json.loads(first.read_text())['created_at']==first_created
    # A missing sibling does not make an otherwise valid scenario unlaunchable.
    sibling=root/'alpha/scenarios/two/scenario.json'
    saved=sibling.read_bytes();sibling.unlink()
    cli('validate',first)
    cli('doctor','alpha-one')
    cli('validate',root/'alpha/demo.json',ok=False)
    sibling.write_bytes(saved)
    # Invalid batch registration leaves the complete previous registry intact.
    data=json.loads(first.read_text());data['id']='beta-one';write(first,data)
    cli('register',root/'alpha/demo.json',ok=False)
    assert (root/'config.toml').read_bytes()==config_before
    # Undeclared scenarios cannot acquire another demo's defaults or identity.
    data['id']='outside';write(root/'alpha/scenarios/extra/scenario.json',data)
    cli('validate',root/'alpha/scenarios/extra/scenario.json',ok=False)
    print('Demos: two owners, shared adapters, parameter overrides, workspace/path resolution, timestamp preservation, sibling isolation and atomic registry validation passed.')
