# What should we call Watson?

The narrator is named John H. Watson in the opening of [*A Study in Scarlet*, part I](https://www.gutenberg.org/files/244/244-h/244-h.htm). In [*The Man with the Twisted Lip*](https://www.gutenberg.org/files/1661/1661-h/1661-h.htm#chap06), his wife offers to send “James” to bed while Watson is present.

Is James another name for Watson, a slip, or something else? The story does
not settle that question. For this example, we will make a deliberately strict
choice: treat John and James as competing first names, and say a person can
have only one. That choice belongs to us, not to Conan Doyle's text.

De Lore Is records the rule and the John claim with the decisions that support
them. A decision is a named ruling: `d-0006` is our modelling rule, and
`d-0001` points to *A Study in Scarlet*. `ratified` records that we accepted
the claim; later rulings can still change what follows. `functional` means at
most one value for each person. [Concepts](../concepts.md) explains the terms.
These are the lines returned by the authoring tools:

```dli
functional first-name ratified d-0006
watson.first-name                      = john                           ratified d-0001
```

Now we stage the alternative, using `d-0003` for *The Man with the Twisted Lip*:

```dli
watson.first-name = james ratified d-0003
```

The check returns this finding (with its other metadata omitted):

```text
continuity.fd-violation: canon breaks the functional dependency on 'first-name': watson carries 2 distinct certain values [james, john]
supporting decisions: d-0001, d-0003
```

That result does not decide which name a writer should use. It identifies the
choice that needs attention and names the sources behind it. A writer could
keep both as names people use instead of treating them as mutually exclusive
first names. The [source notes](../../examples/holmes/sources.md) explain the
decision numbers and the limits of the evidence.
