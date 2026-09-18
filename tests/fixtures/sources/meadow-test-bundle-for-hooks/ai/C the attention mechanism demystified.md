https://momath.org/wp-content/uploads/2023/06/Rohan_Mehta_compressed.pdf

Was described on Twitter as [[T discovery fiction]]

### Key Ideas

^5a8e26

Each word has a context vector.

There are multiple embedding spaces.  One for semantic embeddings, others for queries and keys.

The attention and betting's are multi head. For example, one head might be who is doing some action, and another head might be to whom they are doing it.

### All

Just like a standard layer of neurons, the [[self-attention]] operation is some parameterized, differentiable function we can include in our neural nets. Unlike these layers though, it works on sets of vectors (rather than vectors themselves), and learns some function to linearly combine each element with all the others.

RNNs work on single vectors for context.

[[chat - dot product]]

Consider the sentence <He picked up the tennis ball and found it was wet.= We would probably expect <it= to attend highly to ball= (what <it= is referring to) while <ball= would probably attend highly to <tennis= and picked up= (its type and the action performed on it), but much more weakly to <it= (after all, what new information does it gain from this?). x [[new information]]

Multiple embedding spaces.  A semantic space, a key space and a query space.

To adopt the transformer's lingo, we refer to these key- and lock-representations as queries (key-representations) and keys (lock-representations), respectively. Put simply, a word's key is the information it chooses to expose about itself (the lock), while its query is the information it looks for in other words, when determining how much to attend to them (the key).

[[X single head]]

Contrast, two of the heads in multi had attention. One of the heads might be, who is doing some action, and the other head might be to whom are they doing it

#visualizations #well-explained 