import unittest,unittest.mock,subprocess
from launchagent import reload_agent
class LaunchAgentTests(unittest.TestCase):
 def test_waits_for_unload_and_retries_bootstrap(self):
  codes=iter([0,0,1,5,0]);calls=[]
  def run(args,**kw):calls.append(args);return subprocess.CompletedProcess(args,next(codes))
  with unittest.mock.patch('launchagent.subprocess.run',side_effect=run),unittest.mock.patch('launchagent.time.sleep'):
   reload_agent('gui/501','test','fixture.plist')
  self.assertEqual([x[1] for x in calls],['bootout','print','print','bootstrap','bootstrap'])
 def test_never_bootstraps_over_unstopped_job(self):
  calls=[]
  def run(args,**kw):calls.append(args);return subprocess.CompletedProcess(args,0)
  with unittest.mock.patch('launchagent.subprocess.run',side_effect=run),unittest.mock.patch('launchagent.time.sleep'):
   with self.assertRaises(RuntimeError):reload_agent('gui/501','test','fixture.plist')
  self.assertNotIn('bootstrap',[x[1] for x in calls])
if __name__=='__main__':unittest.main()
