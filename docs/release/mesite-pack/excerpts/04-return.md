# The return from Reichenbach

At the end of [*The Final Problem*](https://www.gutenberg.org/files/834/834-h/834-h.htm), Watson concludes that Holmes died at Reichenbach. In [*The Empty House*](https://www.gutenberg.org/files/108/108-h/108-h), Holmes returns and explains that he escaped. We can revise the earlier ruling while keeping its history.

First, our editorial decision `d-0008` accepts Watson's conclusion, supported
by source `d-0004`. The authoring tool renders the claim as:

```dli
holmes.reichenbach-outcome = died ratified d-0004 d-0008
```

The query returns:

```text
reichenbach-outcome(holmes, died): certain
```

After the return, decision `d-0009` defeats `d-0008`, and a new claim cites
*The Empty House* as `d-0005`. The new claim is:

```dli
holmes.reichenbach-outcome = survived ratified d-0005 d-0009
```

The new query returns:

```text
reichenbach-outcome(holmes, survived): certain
```

When we ask why the old claim no longer holds, the explanation says:

```text
status: dead
support: d-0004, d-0008
d-0009 defeats d-0008
```

Here `dead` describes the old claim's support under the new rulings. The claim
and the earlier decision are still available to explain what changed. This is
a model of a revised account: the later story does not say Holmes died and
came back to life.
