

def foo():
    y = 1
    class A:
        x = y
        def bar(self):
            print(self.x)
    return A

    
