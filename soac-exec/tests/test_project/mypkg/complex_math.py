# __strict__


class Matrix:
    """Simple matrix class supporting multiplication"""

    def __init__(self, rows: list[list[int]]):
        if not rows or not rows[0]:
            raise ValueError("rows must be a non-empty 2D list")
        row_length = len(rows[0])
        for r in rows:
            if len(r) != row_length:
                raise ValueError("all rows must have the same length")
        self.rows = [list(r) for r in rows]

    def __mul__(self, other: "Matrix") -> "Matrix":
        if len(self.rows[0]) != len(other.rows):
            raise ValueError("incompatible matrices")
        m, n, p = len(self.rows), len(self.rows[0]), len(other.rows[0])
        result: list[list[int]] = [[0] * p for _ in range(m)]
        for i in range(m):
            for j in range(p):
                s = 0
                for k in range(n):
                    s += self.rows[i][k] * other.rows[k][j]
                result[i][j] = s
        return Matrix(result)

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Matrix):
            return NotImplemented
        return self.rows == other.rows

    def __repr__(self) -> str:
        return f"Matrix({self.rows!r})"


def fibonacci(n: int) -> int:
    if n < 0:
        raise ValueError("n must be non-negative")
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a


def primes_up_to(n: int) -> list[int]:
    if n < 2:
        return []
    sieve = [True] * (n + 1)
    sieve[0] = sieve[1] = False
    limit = int(n ** 0.5) + 1
    for i in range(2, limit):
        if sieve[i]:
            for j in range(i * i, n + 1, i):
                sieve[j] = False
    return [i for i in range(2, n + 1) if sieve[i]]

