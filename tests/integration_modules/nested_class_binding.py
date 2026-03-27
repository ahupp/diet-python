
class Container:
    class Member:
        pass


def get_member():
    return getattr(Container, "Member", None)


# diet-python: validate

def validate_module(module):
    assert module.get_member() is module.Container.Member
