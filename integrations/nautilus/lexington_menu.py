"""Optional top-level Nautilus menu (requires distribution nautilus-python)."""
import os
from pathlib import Path
import subprocess
import sys
import gi

gi.require_version('Nautilus', '4.0')
from gi.repository import GObject, Nautilus


class LexingtonMenu(GObject.GObject, Nautilus.MenuProvider):
    def get_file_items(self, files):
        paths = [file.get_location().get_path() for file in files]
        if not paths or len(paths) > 32 or any(path is None for path in paths):
            return []
        item = Nautilus.MenuItem(
            name='Lexington::ask', label='看看Lexington怎么说',
            tip='将选中文件路径直接发送给 Lexington',
        )
        data = Path(os.environ.get('XDG_DATA_HOME', Path.home() / '.local/share'))
        helper = data / 'lexington/integrations/nautilus/send.py'
        item.connect('activate', lambda _item: subprocess.Popen(
            [sys.executable, str(helper), *paths], stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        ))
        return [item]
