import sys

import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw

from settings import Settings
from window import Window

settings = Settings()

def on_activate(app):
	Window(app, settings)

app = Adw.Application(application_id='dev.aperte.Memento')
app.connect('activate', on_activate)
app.run(sys.argv)