"""Local browser example. Stdout is reserved for the Demio protocol."""
import functools
import http.server
import json
import os
from pathlib import Path
import sys
import threading

browser = context = page = server = playwright = None

def focus(selector):
    node = page.locator(selector)
    node.evaluate("el => el.scrollIntoView({behavior:'smooth',block:'center'})")
    page.wait_for_timeout(650)
    node.evaluate("el => {el.style.outline='3px solid #6474ff'; setTimeout(()=>el.style.outline='',1400)}")
    page.wait_for_timeout(500)

def cleanup():
    if context:
        context.close()
    if playwright:
        playwright.stop()
    if server:
        server.shutdown()
        server.server_close()

for line in sys.stdin:
    request = json.loads(line)
    try:
        op, step = request['op'], request.get('step')
        if op == 'check':
            from playwright.sync_api import sync_playwright
            with sync_playwright() as probe:
                if not Path(probe.chromium.executable_path).exists():
                    raise RuntimeError('Run .venv/bin/python -m playwright install chromium')
        elif op == 'prepare':
            from playwright.sync_api import sync_playwright
            handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=os.getcwd())
            server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), handler)
            threading.Thread(target=server.serve_forever, daemon=True).start()
            playwright = sync_playwright().start()
            context = playwright.chromium.launch_persistent_context(str(Path(request['session_dir'])/'browser'), headless=os.environ.get('DEMIO_HEADLESS') == '1', viewport={'width':1280,'height':800})
            page = context.pages[0]
            page.goto(f'http://127.0.0.1:{server.server_port}')
        elif op in ('run', 'preview'):
            selectors = {'intro':'#role','fill':'#form','submit':'#status','approve':'#approval','close':'#status'}
            if step not in selectors:
                raise RuntimeError('Unknown step')
            if op == 'run':
                if step == 'fill':
                    focus('#form')
                    page.locator('#summary').press_sequentially('New laptop for the design team', delay=45)
                    page.locator('#reason').press_sequentially('Our current laptop needs replacement.', delay=35)
                    assert page.locator('#summary').input_value() == 'New laptop for the design team'
                    assert page.locator('#reason').input_value() == 'Our current laptop needs replacement.'
                elif step == 'submit':
                    focus('#submit')
                    page.locator('#submit').click()
                    page.locator('#status').filter(has_text='Awaiting approval').wait_for()
                elif step == 'approve':
                    page.locator('#role').select_option('Approver')
                    focus('#approval')
                    page.wait_for_timeout(900)
                    page.locator('#approve').click()
                    page.locator('#status').filter(has_text='Approved').wait_for()
                elif step == 'close':
                    page.locator('#role').select_option('Requester')
                    assert page.locator('#status').inner_text() == 'Approved'
            focus(selectors[step])
        elif op == 'cleanup':
            cleanup()
        else:
            raise RuntimeError('Unknown operation')
        print(json.dumps({'id':request['id'],'event':'done','ok':True}), flush=True)
        if op == 'cleanup':
            break
    except Exception as error:
        print(json.dumps({'id':request['id'],'event':'done','ok':False,'error':str(error)}), flush=True)
