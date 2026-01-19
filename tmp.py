"""Utilities to support packages."""
import __dp__
_dp_import_1 = __dp__.import_("collections", __spec__, ["namedtuple"])
namedtuple = __dp__.import_attr(_dp_import_1, "namedtuple")
del _dp_import_1
_dp_import_2 = __dp__.import_("functools", __spec__, ["singledispatch"])
simplegeneric = __dp__.import_attr(_dp_import_2, "singledispatch")
del _dp_import_2
importlib = __dp__.import_("importlib", __spec__)
importlib = __dp__.import_("importlib.util", __spec__)
importlib = __dp__.import_("importlib.machinery", __spec__)
os = __dp__.import_("os", __spec__)
os = __dp__.import_("os.path", __spec__)
sys = __dp__.import_("sys", __spec__)
__all__ = __dp__.list(('get_importer', 'iter_importers', 'walk_packages', 'iter_modules', 'get_data', 'read_code', 'extend_path', 'ModuleInfo'))
ModuleInfo = namedtuple('ModuleInfo', 'module_finder name ispkg')
ModuleInfo.__doc__ = 'A namedtuple with minimal info about a module.'
def read_code(stream):
    marshal = __dp__.import_("marshal", __spec__)
    magic = stream.read(4)
    if magic != importlib.util.MAGIC_NUMBER:
        return None
    stream.read(12)
    return marshal.load(stream)
def walk_packages(path=None, prefix='', onerror=None):
    """Yields ModuleInfo for all modules recursively\n    on path, or, if path is None, all accessible modules.\n\n    'path' should be either None or a list of paths to look for\n    modules in.\n\n    'prefix' is a string to output on the front of every module name\n    on output.\n\n    Note that this function must import all *packages* (NOT all\n    modules!) on the given path, in order to access the __path__\n    attribute to find submodules.\n\n    'onerror' is a function which gets called with one argument (the\n    name of the package which was being imported) if any exception\n    occurs while trying to import a package.  If no onerror function is\n    supplied, ImportErrors are caught and ignored, while all other\n    exceptions are propagated, terminating the search.\n\n    Examples:\n\n    # list all modules python can access\n    walk_packages()\n\n    # list all submodules of ctypes\n    walk_packages(ctypes.__path__, ctypes.__name__+'.')\n    """

    def seen(p, m={}):
        if p in m:
            return True
        __dp__.setitem(m, p, True)
    _dp_iter_3 = __dp__.iter(iter_modules(path, prefix))
    try:
        while True:
            info = __dp__.next(_dp_iter_3)
            yield info
            if info.ispkg:
                try:
                    __import__(info.name)
                except ImportError:
                    if onerror is not None:
                        onerror(info.name)
                except Exception:
                    if onerror is not None:
                        onerror(info.name)
                    else:
                        raise
                else:
                    path = getattr(sys.modules[info.name], '__path__', None) or []
                    path = [p for p in path if not seen(p)]
                    yield from walk_packages(path, info.name + '.', onerror)
    except:
        if __dp__.exception_matches(__dp__.current_exception(), StopIteration):
            pass
        else:
            raise
def iter_modules(path=None, prefix=''):
    """Yields ModuleInfo for all submodules on path,\n    or, if path is None, all top-level modules on sys.path.\n\n    'path' should be either None or a list of paths to look for\n    modules in.\n\n    'prefix' is a string to output on the front of every module name\n    on output.\n    """
    if path is None:
        importers = iter_importers()
    else:
        if isinstance(path, str):
            raise ValueError("path must be None or list of paths to look for " "modules in")
        else:
            importers = map(get_importer, path)
    yielded = __dp__.dict()
    _dp_iter_4 = __dp__.iter(importers)
    try:
        while True:
            i = __dp__.next(_dp_iter_4)
            for name, ispkg in iter_importer_modules(i, prefix):
                if name not in yielded:
                    yielded[name] = 1
                    yield ModuleInfo(i, name, ispkg)
    except:
        if __dp__.exception_matches(__dp__.current_exception(), StopIteration):
            pass
        else:
            raise
def iter_importer_modules(importer, prefix=''):
    if not hasattr(importer, 'iter_modules'):
        return []
    return importer.iter_modules(prefix)
iter_importer_modules = simplegeneric(iter_importer_modules)
def _iter_file_finder_modules(importer, prefix=''):
    if importer.path is None or not os.path.isdir(importer.path):
        return
    yielded = __dp__.dict()
    inspect = __dp__.import_("inspect", __spec__)
    try:
        filenames = os.listdir(importer.path)
    except:
        if __dp__.exception_matches(__dp__.current_exception(), OSError):
            filenames = []
        else:
            raise
    filenames.sort()
    _dp_iter_5 = __dp__.iter(filenames)
    try:
        while True:
            fn = __dp__.next(_dp_iter_5)
            modname = inspect.getmodulename(fn)
            if modname == '__init__' or modname in yielded:
                continue
            path = os.path.join(importer.path, fn)
            ispkg = False
            if not modname and os.path.isdir(path) and '.' not in fn:
                modname = fn
                try:
                    dircontents = os.listdir(path)
                except OSError:
                    dircontents = []
                for fn in dircontents:
                    subname = inspect.getmodulename(fn)
                    if subname == '__init__':
                        ispkg = True
                        break
                else:
                    continue
            if modname and '.' not in modname:
                yielded[modname] = 1
                yield prefix + modname, ispkg
    except:
        if __dp__.exception_matches(__dp__.current_exception(), StopIteration):
            pass
        else:
            raise
iter_importer_modules.register(importlib.machinery.FileFinder, _iter_file_finder_modules)
try:
    import zipimport
    from zipimport import zipimporter

    def iter_zipimport_modules(importer, prefix=''):
        dirlist = sorted(zipimport._zip_directory_cache[importer.archive])
        _prefix = importer.prefix
        plen = len(_prefix)
        yielded = {}
        import inspect
        for fn in dirlist:
            if not fn.startswith(_prefix):
                continue
            fn = fn[plen:].split(os.sep)
            if len(fn) == 2 and fn[1].startswith('__init__.py'):
                if fn[0] not in yielded:
                    yielded[fn[0]] = 1
                    yield prefix + fn[0], True
            if len(fn) != 1:
                continue
            modname = inspect.getmodulename(fn[0])
            if modname == '__init__':
                continue
            if modname and '.' not in modname and modname not in yielded:
                yielded[modname] = 1
                yield prefix + modname, False
    iter_importer_modules.register(zipimporter, iter_zipimport_modules)
except:
    if __dp__.exception_matches(__dp__.current_exception(), ImportError):
        pass
    else:
        raise
def get_importer(path_item):
    """Retrieve a finder for the given path item\n\n    The returned finder is cached in sys.path_importer_cache\n    if it was newly created by a path hook.\n\n    The cache (or part of it) can be cleared manually if a\n    rescan of sys.path_hooks is necessary.\n    """
    path_item = os.fsdecode(path_item)
    try:
        importer = sys.path_importer_cache[path_item]
    except:
        if __dp__.exception_matches(__dp__.current_exception(), KeyError):
            for path_hook in sys.path_hooks:
                try:
                    importer = path_hook(path_item)
                    sys.path_importer_cache.setdefault(path_item, importer)
                    break
                except ImportError:
                    pass
            else:
                importer = None
        else:
            raise
    return importer
def iter_importers(fullname=""):
    """Yield finders for the given module name\n\n    If fullname contains a '.', the finders will be for the package\n    containing fullname, otherwise they will be all registered top level\n    finders (i.e. those on both sys.meta_path and sys.path_hooks).\n\n    If the named module is in a package, that package is imported as a side\n    effect of invoking this function.\n\n    If no module name is specified, all top level finders are produced.\n    """
    if fullname.startswith('.'):
        msg = "Relative module name {!r} not supported".format(fullname)
        raise ImportError(msg)
    if '.' in fullname:
        pkg_name = fullname.rpartition(".")[0]
        pkg = importlib.import_module(pkg_name)
        path = getattr(pkg, '__path__', None)
        if path is None:
            return
    else:
        yield from sys.meta_path
        path = sys.path
    _dp_iter_6 = __dp__.iter(path)
    try:
        while True:
            item = __dp__.next(_dp_iter_6)
            yield get_importer(item)
    except:
        if __dp__.exception_matches(__dp__.current_exception(), StopIteration):
            pass
        else:
            raise
def extend_path(path, name):
    """Extend a package's path.\n\n    Intended use is to place the following code in a package's __init__.py:\n\n        from pkgutil import extend_path\n        __path__ = extend_path(__path__, __name__)\n\n    For each directory on sys.path that has a subdirectory that\n    matches the package name, add the subdirectory to the package's\n    __path__.  This is useful if one wants to distribute different\n    parts of a single logical package as multiple directories.\n\n    It also looks for *.pkg files beginning where * matches the name\n    argument.  This feature is similar to *.pth files (see site.py),\n    except that it doesn't special-case lines starting with 'import'.\n    A *.pkg file is trusted at face value: apart from checking for\n    duplicates, all entries found in a *.pkg file are added to the\n    path, regardless of whether they are exist the filesystem.  (This\n    is a feature.)\n\n    If the input path is not a list (as is the case for frozen\n    packages) it is returned unchanged.  The input path is not\n    modified; an extended copy is returned.  Items are only appended\n    to the copy at the end.\n\n    It is assumed that sys.path is a sequence.  Items of sys.path that\n    are not (unicode or 8-bit) strings referring to existing\n    directories are ignored.  Unicode items of sys.path that cause\n    errors when used as filenames may cause this function to raise an\n    exception (in line with os.path.isdir() behavior).\n    """
    if not isinstance(path, list):
        return path
    sname_pkg = __dp__.add(name, ".pkg")
    path = __dp__.getitem(path, :)
    _dp_tmp_7 = name.rpartition('.')
    try:
        _dp_tmp_8 = __dp__.unpack(_dp_tmp_7, (True, True, True))
        parent_package = __dp__.getitem(_dp_tmp_8, 0)
        _ = __dp__.getitem(_dp_tmp_8, 1)
        final_name = __dp__.getitem(_dp_tmp_8, 2)
    finally:
        _dp_tmp_8 = None
        _dp_tmp_7 = None
    if parent_package:
        try:
            search_path = sys.modules[parent_package].__path__
        except (KeyError, AttributeError):
            return path
    else:
        search_path = sys.path
    _dp_iter_9 = __dp__.iter(search_path)
    try:
        while True:
            dir = __dp__.next(_dp_iter_9)
            if not isinstance(dir, str):
                continue
            finder = get_importer(dir)
            if finder is not None:
                portions = []
                if hasattr(finder, 'find_spec'):
                    spec = finder.find_spec(final_name)
                    if spec is not None:
                        portions = spec.submodule_search_locations or []
                elif hasattr(finder, 'find_loader'):
                    _, portions = finder.find_loader(final_name)
                for portion in portions:
                    if portion not in path:
                        path.append(portion)
            pkgfile = os.path.join(dir, sname_pkg)
            if os.path.isfile(pkgfile):
                try:
                    f = open(pkgfile)
                except OSError as msg:
                    sys.stderr.write("Can't open %s: %s\n" % (pkgfile, msg))
                else:
                    with f:
                        for line in f:
                            line = line.rstrip('\n')
                            if not line or line.startswith('#'):
                                continue
                            path.append(line)
    except:
        if __dp__.exception_matches(__dp__.current_exception(), StopIteration):
            pass
        else:
            raise
    return path
def get_data(package, resource):
    """Get a resource from a package.\n\n    This is a wrapper round the PEP 302 loader get_data API. The package\n    argument should be the name of a package, in standard module format\n    (foo.bar). The resource argument should be in the form of a relative\n    filename, using '/' as the path separator. The parent directory name '..'\n    is not allowed, and nor is a rooted name (starting with a '/').\n\n    The function returns a binary string, which is the contents of the\n    specified resource.\n\n    For packages located in the filesystem, which have already been imported,\n    this is the rough equivalent of\n\n        d = os.path.dirname(sys.modules[package].__file__)\n        data = open(os.path.join(d, resource), 'rb').read()\n\n    If the package cannot be located or loaded, or it uses a PEP 302 loader\n    which does not support get_data(), then None is returned.\n    """
    spec = importlib.util.find_spec(package)
    if spec is None:
        return None
    loader = spec.loader
    if loader is None or not hasattr(loader, 'get_data'):
        return None
    _dp_target_10 = sys.modules.get(package)
    if not _dp_target_10:
        _dp_target_10 = importlib._bootstrap._load(spec)
    mod = _dp_target_10
    if mod is None or not hasattr(mod, '__file__'):
        return None
    parts = resource.split('/')
    parts.insert(0, os.path.dirname(mod.__file__))
    resource_name = os.path.join(*parts)
    return loader.get_data(resource_name)
_NAME_PATTERN = None
def resolve_name(name):
    """\n    Resolve a name to an object.\n\n    It is expected that `name` will be a string in one of the following\n    formats, where W is shorthand for a valid Python identifier and dot stands\n    for a literal period in these pseudo-regexes:\n\n    W(.W)*\n    W(.W)*:(W(.W)*)?\n\n    The first form is intended for backward compatibility only. It assumes that\n    some part of the dotted name is a package, and the rest is an object\n    somewhere within that package, possibly nested inside other objects.\n    Because the place where the package stops and the object hierarchy starts\n    can't be inferred by inspection, repeated attempts to import must be done\n    with this form.\n\n    In the second form, the caller makes the division point clear through the\n    provision of a single colon: the dotted name to the left of the colon is a\n    package to be imported, and the dotted name to the right is the object\n    hierarchy within that package. Only one import is needed in this form. If\n    it ends with the colon, then a module object is returned.\n\n    The function will return an object (which might be a module), or raise one\n    of the following exceptions:\n\n    ValueError - if `name` isn't in a recognised format\n    ImportError - if an import failed when it shouldn't have\n    AttributeError - if a failure occurred when traversing the object hierarchy\n                     within the imported package to get to the desired object.\n    """
    global _NAME_PATTERN
    if _NAME_PATTERN is None:
        import re
        dotted_words = r'(?!\d)(\w+)(\.(?!\d)(\w+))*'
        _NAME_PATTERN = re.compile(f'^(?P<pkg>{dotted_words})' f'(?P<cln>:(?P<obj>{dotted_words})?)?$', re.UNICODE)
    m = _NAME_PATTERN.match(name)
    if not m:
        raise ValueError(f'invalid format: {name!r}')
    gd = m.groupdict()
    if gd.get('cln'):
        mod = importlib.import_module(gd['pkg'])
        parts = gd.get('obj')
        parts = parts.split('.') if parts else []
    else:
        parts = name.split('.')
        modname = parts.pop(0)
        mod = importlib.import_module(modname)
        while parts:
            p = parts[0]
            s = f'{modname}.{p}'
            try:
                mod = importlib.import_module(s)
                parts.pop(0)
                modname = s
            except ImportError:
                break
    result = mod
    _dp_iter_11 = __dp__.iter(parts)
    try:
        while True:
            p = __dp__.next(_dp_iter_11)
            result = getattr(result, p)
    except:
        if __dp__.exception_matches(__dp__.current_exception(), StopIteration):
            pass
        else:
            raise
    return result
__dp__.cleanup_dp_globals(globals())
