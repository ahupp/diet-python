
# complicated generator


def complicated(a):
    for i in a:
        try:
            j = i + 1
            yield j
        except Exception:
            print("oops")


# ==


# -- pre-bb --
def _dp_module_init():

    def complicated(a):
        # pc 0
        a_iter = iter(a)
        try:
            #jump(loop0)
            while True:
                # block loop0
                i = __dp__.next(a_iter)
                try:
                    j = __dp__.add(i, 1)
                    # pc 1
                    yield j
                except:
                    if __dp__.exception_matches(__dp__.current_exception(), Exception):
                        print("oops")
                    else:
                        raise
        except StopIteration:
            pass


def complicated_resume(self, value, exc):
    if exc is None:
        return complicated_resume_value(self, value)
    else:
        return complicated_resume_exc(self, value, exc)

def complicated_resume_value(self, value):

# -- bb --
def _dp_bb_complicated_done(_dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
    )
    __dp__.setattr(_dp_self, "_pc", __dp__._GEN_PC_DONE)
    return __dp__.ret(None)


def _dp_bb_complicated_invalid(_dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
    )
    return __dp__.raise_(RuntimeError("invalid generator pc: {}".format(_dp_self._pc)))


def _dp_bb_complicated_resume_0(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_iter_1
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_iter_1 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
        _dp_iter_1.take(),
    )
    return __dp__.jump(
        _dp_bb_complicated_internal_6,
        (_dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_iter_1),
    )


def _dp_bb_complicated_internal_0(
    _dp_self, _dp_send_value, _dp_resume_exc, i, _dp_try_exc_7, _dp_iter_1
):
    _dp_self, _dp_send_value, _dp_resume_exc, i, _dp_try_exc_7, _dp_iter_1 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        i.take(),
        _dp_try_exc_7.take(),
        _dp_iter_1.take(),
    )
    j = __dp__.add(i, 1)
    __dp__.setattr(_dp_self, "_pc", 3)
    __dp__.frame_store(_dp_self, "_dp_self", _dp_self)
    __dp__.frame_store(_dp_self, "_dp_try_exc_7", _dp_try_exc_7)
    __dp__.frame_store(_dp_self, "_dp_iter_1", _dp_iter_1)
    return __dp__.ret(j)


def _dp_bb_complicated_internal_1(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_1.take(),
        _dp_try_exc_7.take(),
    )
    _dp_try_exc_7 = __dp__.DELETED
    return __dp__.jump(
        _dp_bb_complicated_internal_6,
        (_dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_iter_1),
    )


def _dp_bb_complicated_internal_2(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_1.take(),
        _dp_try_exc_7.take(),
    )
    print("oops")
    return __dp__.jump(
        _dp_bb_complicated_internal_1,
        (_dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7),
    )


def _dp_bb_complicated_internal_3(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
    )
    return __dp__.raise_(_dp_try_exc_7)


def _dp_bb_complicated_internal_4(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_1.take(),
        _dp_try_exc_7.take(),
    )
    return __dp__.brif(
        __dp__.exception_matches(_dp_try_exc_7, Exception),
        _dp_bb_complicated_internal_2,
        (_dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7),
        _dp_bb_complicated_internal_3,
        (_dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7),
    )


def _dp_bb_complicated_resume_1(
    _dp_self, _dp_send_value, _dp_resume_exc, i, _dp_try_exc_7, _dp_iter_1
):
    _dp_self, _dp_send_value, _dp_resume_exc, i, _dp_try_exc_7, _dp_iter_1 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        i.take(),
        _dp_try_exc_7.take(),
        _dp_iter_1.take(),
    )
    return __dp__.try_jump_term(
        _dp_bb_complicated_internal_0,
        (_dp_self, _dp_send_value, _dp_resume_exc, i, _dp_try_exc_7, _dp_iter_1),
        (_dp_bb_complicated_resume_0, _dp_bb_complicated_internal_0),
        _dp_bb_complicated_internal_4,
        (_dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1),
        True,
        (
            _dp_bb_complicated_internal_1,
            _dp_bb_complicated_internal_2,
            _dp_bb_complicated_internal_3,
            _dp_bb_complicated_internal_4,
        ),
        None,
        (),
        False,
        (),
        None,
    )


def _dp_bb_complicated_internal_5(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_tmp_2, _dp_iter_1
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_tmp_2, _dp_iter_1 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
        _dp_tmp_2.take(),
        _dp_iter_1.take(),
    )
    i = _dp_tmp_2
    _dp_tmp_2 = None
    return __dp__.jump(
        _dp_bb_complicated_resume_1,
        (_dp_self, _dp_send_value, _dp_resume_exc, i, _dp_try_exc_7, _dp_iter_1),
    )


def _dp_bb_complicated_internal_6(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_iter_1
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_iter_1 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
        _dp_iter_1.take(),
    )
    _dp_tmp_2 = __dp__.next_or_sentinel(_dp_iter_1)
    return __dp__.brif(
        __dp__.is_(_dp_tmp_2, __dp__.ITER_COMPLETE),
        _dp_bb_complicated_internal_7,
        (_dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7),
        _dp_bb_complicated_internal_5,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            _dp_try_exc_7,
            _dp_tmp_2,
            _dp_iter_1,
        ),
    )


def _dp_bb_complicated_resume_2(
    _dp_self, _dp_send_value, _dp_resume_exc, a, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, a, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        a.take(),
        _dp_try_exc_7.take(),
    )
    _dp_try_exc_7 = __dp__.DELETED
    _dp_iter_1 = __dp__.iter(a)
    return __dp__.jump(
        _dp_bb_complicated_internal_6,
        (_dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_iter_1),
    )


def _dp_bb_complicated_internal_7(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
    )
    __dp__.setattr(_dp_self, "_pc", __dp__._GEN_PC_DONE)
    return __dp__.ret(None)


def _dp_bb__dp_module_init_start():
    __dp__.store_global(
        globals(),
        "complicated",
        __dp__.def_gen(
            11,
            (
                _dp_bb_complicated_done,
                _dp_bb_complicated_invalid,
                _dp_bb_complicated_resume_0,
                _dp_bb_complicated_internal_0,
                _dp_bb_complicated_internal_1,
                _dp_bb_complicated_internal_2,
                _dp_bb_complicated_internal_3,
                _dp_bb_complicated_internal_4,
                _dp_bb_complicated_resume_1,
                _dp_bb_complicated_internal_5,
                _dp_bb_complicated_internal_6,
                _dp_bb_complicated_resume_2,
                _dp_bb_complicated_internal_7,
            ),
            (-1, -1, 8, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1),
            "complicated",
            "complicated",
            ("a", "_dp_try_exc_7"),
            (("a", None, __dp__.NO_DEFAULT),),
            __name__,
        ),
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start
