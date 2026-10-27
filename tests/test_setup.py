import importlib.util,unittest.mock,json,pathlib,subprocess,tempfile,unittest,os
ROOT=pathlib.Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('setup',ROOT/'scripts/setup.py');setup=importlib.util.module_from_spec(spec);spec.loader.exec_module(setup)
class SetupTests(unittest.TestCase):
 def test_fresh_install_and_rerun_preserve_private_state(self):
  with tempfile.TemporaryDirectory(prefix='setup test ') as d:
   h=pathlib.Path(d);cmd=['python3',str(ROOT/'scripts/setup.py'),'--non-interactive','--no-start','--home',d]
   subprocess.run(cmd,check=True,stdout=subprocess.DEVNULL)
   hub=h/('Library/Application Support' if setup.platform.system()=='Darwin' else '.local/share')/'ModelHub';token=hub/'.local/hub.key';before=token.read_text();reg=hub/'.local/providers.json';r=json.loads(reg.read_text());r['note']='fixture-preserve';reg.write_text(json.dumps(r));subprocess.run(cmd,check=True,stdout=subprocess.DEVNULL)
   self.assertEqual(before,token.read_text());self.assertEqual(json.loads(reg.read_text())['note'],'fixture-preserve');self.assertEqual(token.stat().st_mode&0o777,0o600);self.assertFalse((h/'.pi').exists());self.assertTrue((h/'.local/bin/modelhub').exists())
 def test_both_platform_service_files_reference_shared_hub(self):
  with tempfile.TemporaryDirectory(prefix='service test ') as d:
   h=pathlib.Path(d);hub=h/'Hub with spaces';quota=h/'Quota with spaces';hub.mkdir();quota.mkdir()
   for system in ['Darwin','Linux']:setup.install_services('/path with spaces/node',hub,quota,h,False,system)
   p=h/'Library/LaunchAgents/local.model-quota.manager.plist';v=setup.plistlib.loads(p.read_bytes());self.assertEqual(v['EnvironmentVariables']['MODEL_HUB_DATA_DIR'],str(hub/'.local'))
   unit=(h/'.config/systemd/user/local.model-access.hub.service').read_text();self.assertIn('ExecStart="/path with spaces/node"',unit)
 def test_agent_setup_preserves_preferences_and_uses_only_hub_key(self):
  with tempfile.TemporaryDirectory(prefix='agent setup ') as d:
   h=pathlib.Path(d);hub=h/'ModelHub';quota=h/'Quota';hub.mkdir();quota.mkdir()
   import shutil
   shutil.copytree(ROOT/'model-access',hub,dirs_exist_ok=True)
   reg={'proxy':'','providers':{'local-glm':{'prefix':'glm','name':'Fixture','baseURL':'https://open.bigmodel.cn/api/coding/paas/v4','billing':'fixture','enabled':True,'keyFile':str(hub/'.local/key'),'models':[{'id':'glm-5.3','name':'GLM 5.3','contextWindow':128000,'maxTokens':128}]}}}
   setup.write(hub/'.local/providers.json',reg);setup.write(hub/'.local/hub.key','HUB_FIXTURE_ONLY');setup.write(hub/'.local/key','UPSTREAM_FIXTURE_ONLY');setup.write(h/'.claude/settings.json',{'theme':'dark','permissions':{'defaultMode':'manual'}})
   node=shutil.which('node')
   with unittest.mock.patch.object(setup.shutil,'which',return_value='/fixture/claude'):
    setup.connect_agents(node,hub,h,reg,hub/'.local/backups')
   c=json.loads((h/'.claude/settings.json').read_text());self.assertEqual(c['theme'],'dark');self.assertEqual(c['permissions']['defaultMode'],'manual');self.assertEqual(c['model'],'glm/glm-5.3');self.assertIn('apiKeyHelper',c);self.assertNotIn('UPSTREAM_FIXTURE_ONLY',json.dumps(c));self.assertTrue((h/'.pi/agent/models.json').exists())
 def test_secret_validation_rejects_non_tty(self):
  with self.assertRaises(ValueError):setup.hidden('fixture')
if __name__=='__main__':unittest.main()
