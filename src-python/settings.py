import json
from os import path

# TODO: This implementation is temporary, will be replaced with GtkSettings
class Settings():
	"""Settings manager for reading and saving application settings to JSON file"""
	def __init__(self) -> None:
		self._dir_path = path.split(__file__)[0]
		self._file_name = 'settings.json'
		self._all_settings: dict = {}
		self._load()

	@property
	def last_meme_directory(self) -> bool:
		"""Returns last opened directory if still exists"""
		last_dir = self._all_settings.get('last_meme_directory', None)
		if path.isdir(last_dir):
			return last_dir
		else:
			return False

	@last_meme_directory.setter
	def last_meme_directory(self, value: str | None):
		"""Sets last opened directory"""
		self._all_settings['last_meme_directory'] = value
		self._save()

	def _load(self):
		try:
			settings_path = path.join(self._dir_path, self._file_name)

			if not path.exists(settings_path):
				print('[W] Settings file not found')
				return

			with open(settings_path, 'r') as file:
				self._all_settings = json.load(file)

		except IOError:
			print('[E] Cannot read settings file')

	def _save(self):
		try:
			settings_path = path.join(self._dir_path, self._file_name)

			with open(settings_path, 'w') as file:
				json.dump(self._all_settings, file)

		except IOError:
			print('[E] Cannot read settings file')