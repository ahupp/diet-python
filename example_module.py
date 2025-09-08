import sys
from sys import exc_info as ei


@foo
@bar(1,2)
def add(a, b):
    return a + b

class A:
    b = 1

    def __init__(self):
        self.arr = [1,2,3]
    
    def c(self, d):
        return add(d, 2)

    async def test_aiter(self):
        for i in range(10):
            yield i
    
    async def d(self):
        async for i in self.test_aiter():
            print(i)

def ff():
  a = A()
  a.b = 5

  c = object()
  c.a = a

c = ff()

del c.a.b, c.a.arr[0], c

x = [i + 1 for i in range(5) if i % 2 == 0]
y = {i + 1 for i in range(5) if i % 2 == 0}
z = (i + 1 for i in range(5) if i % 2 == 0)


   
