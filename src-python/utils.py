from threading import Thread
from typing import Callable, List
import os, json

from unidecode import unidecode

try:
	import easyocr
	reader = easyocr.Reader(['pl','en'], gpu=True)
except ModuleNotFoundError:
	print('[W] EasyOCR not installed, OCR functionality will not be available')

def is_ocr_available() -> bool:
	"""Returns information if OCR feature is available"""
	try:
		return bool(reader)
	except NameError:
		return False

def get_list_of_files(path: str, types: None | List[str] = None) -> List[str]:
	"""Returns flat list of all image paths under specified path (including subdirectories)"""
	file_paths = []
	for root, dirs, files in os.walk(path):
		dirs[:] = [d for d in dirs if not d.startswith('.')]
		for file in files:
			file_path = os.path.join(root, file)
			if (types is None):
				file_paths.append(file_path)
			elif os.path.splitext(file_path)[1] in types:
				file_paths.append(file_path)
	return file_paths

def scan_ocr_image(path: str, callback: Callable[[], List[str]]):
	"""Runs OCR on image under specified path, then calls callback"""
	Thread(target=_scan_ocr_thread, args=(path, callback)).start()

# TODO: Move to separate service (API)
def _scan_ocr_thread(path: str, callback: Callable[[], List[str]]):
	print('[I] Start OCR process')
	words = []
	for word in reader.readtext(path, detail=0):
		for subword in word.split():
			words.append(unidecode(subword.strip().lower()))
	print(f'[I] Found words: {words}')
	callback(words)
	dir_path, filename = os.path.split(path)
	meta_path = os.path.join(dir_path, '.cache', filename + '.json')
	try:
		# TODO: Rewrite to avoid overwriting entire cache file
		with open(meta_path, 'w') as file:
			json.dump({'tags': words}, file)
	except IOError as e:
		print(f'[E] Failed to save cache file: "{meta_path}"', e.strerror)