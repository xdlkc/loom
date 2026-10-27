import hashlib,importlib.util,io,pathlib,sys,tarfile,tempfile,unittest,unittest.mock
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parents[1]/'scripts'))
import node_runtime as runtime
class RuntimeTests(unittest.TestCase):
 def test_manifest_picks_correct_platform(self):
  name='node-v24.22.1-darwin-arm64.tar.gz';self.assertEqual(runtime.artifact('a'*64+'  '+name,'Darwin','arm64'),(name,'a'*64))
  with self.assertRaises(ValueError):runtime.artifact('a'*64+'  '+name,'Linux','x86_64')
 def test_archive_path_escape_rejected(self):
  with tempfile.TemporaryDirectory() as d:
   p=pathlib.Path(d)/'bad.tar.gz'
   with tarfile.open(p,'w:gz') as t:
    m=tarfile.TarInfo('../escape');m.size=0;t.addfile(m,io.BytesIO())
   with self.assertRaises(ValueError):runtime.unpack(p,pathlib.Path(d),'node-v24.0.0-linux-x64')
 def test_old_node_downloads_verified_runtime_and_reuses_it(self):
  with tempfile.TemporaryDirectory() as d:
   h=pathlib.Path(d);top='node-v24.22.1-linux-x64';archive=h/'fixture.tar.gz'
   with tarfile.open(archive,'w:gz') as t:
    data=b'#!/bin/sh\necho 24\n';m=tarfile.TarInfo(top+'/bin/node');m.size=len(data);m.mode=0o755;t.addfile(m,io.BytesIO(data))
   blob=archive.read_bytes();digest=hashlib.sha256(blob).hexdigest()
   def download(url,p):p.write_bytes((digest+'  '+top+'.tar.gz\n').encode() if url.endswith('.txt') else blob)
   with unittest.mock.patch.object(runtime.shutil,'which',return_value=None),unittest.mock.patch.object(runtime.platform,'machine',return_value='x86_64'),unittest.mock.patch.object(runtime,'download',side_effect=download) as dl:
    node=runtime.ensure_node(h,'Linux');self.assertTrue(pathlib.Path(node).exists());self.assertEqual(dl.call_count,2);dl.reset_mock();self.assertEqual(runtime.ensure_node(h,'Linux'),node);dl.assert_not_called()
 def test_official_failure_falls_back_using_pinned_digest(self):
  with tempfile.TemporaryDirectory() as d:
   h=pathlib.Path(d);top='node-v24.22.1-linux-x64';archive=h/'fixture.tar.gz'
   with tarfile.open(archive,'w:gz') as t:
    data=b'#!/bin/sh\necho 24\n';m=tarfile.TarInfo(top+'/bin/node');m.size=len(data);m.mode=0o755;t.addfile(m,io.BytesIO(data))
   blob=archive.read_bytes();pin=hashlib.sha256(blob).hexdigest()+'  '+top+'.tar.gz';urls=[]
   def download(url,p):
    urls.append(url)
    if url.startswith(runtime.ORIGIN):raise ValueError('fixture connection failure')
    p.write_bytes(blob)
   with unittest.mock.patch.object(runtime.shutil,'which',return_value=None),unittest.mock.patch.object(runtime.platform,'machine',return_value='x86_64'),unittest.mock.patch.object(runtime,'PINNED',pin),unittest.mock.patch.object(runtime,'download',side_effect=download):
    self.assertTrue(pathlib.Path(runtime.ensure_node(h,'Linux')).exists())
   self.assertTrue(urls[0].startswith(runtime.ORIGIN));self.assertTrue(urls[1].startswith(runtime.MIRROR))
 def test_checksum_failure_installs_nothing(self):
  with tempfile.TemporaryDirectory() as d:
   h=pathlib.Path(d)
   def download(url,p):p.write_bytes(('a'*64+'  node-v24.22.1-linux-x64.tar.gz\n').encode() if url.endswith('.txt') else b'corrupt')
   with unittest.mock.patch.object(runtime.shutil,'which',return_value=None),unittest.mock.patch.object(runtime.platform,'machine',return_value='x86_64'),unittest.mock.patch.object(runtime,'download',side_effect=download):
    with self.assertRaisesRegex(ValueError,'SHA-256'):runtime.ensure_node(h,'Linux')
   self.assertEqual(list(h.rglob('bin/node')),[])
 def test_dry_run_does_not_download_or_create_directories(self):
  with tempfile.TemporaryDirectory() as d:
   h=pathlib.Path(d)
   with unittest.mock.patch.object(runtime.shutil,'which',return_value=None),unittest.mock.patch.object(runtime,'download') as dl:
    self.assertIsNone(runtime.ensure_node(h,'Linux',dry_run=True));dl.assert_not_called()
   self.assertEqual(list(h.iterdir()),[])
if __name__=='__main__':unittest.main()
