In a transformer, when you positionally encode the token ids for a sequence, what kind of values do you get?  Is it one value, like a float, per token?  Is it a vector?

<hr class="__chatgpt_plugin">

role::assistant

In a transformer, when you positionally encode the token ids for a sequence, you get a vector of values for each token. This vector is usually the same size as the embedding dimension. The positional encoding adds information about the relative or absolute position of the tokens in the sequence. The values in the vector are generated using a specific function, often a combination of sine and cosine functions. This allows the model to capture the order of the words in the sequence, which is important for understanding the meaning of a sentence. X [[positional encoding]]

<hr class="__chatgpt_plugin">

role::user

What values should I expect to see in the vector?  Floats from -1 to 1?  Something else?



<hr class="__chatgpt_plugin">

role::assistant

Yes, you should expect to see floating point values in the vector. The values are generated using sine and cosine functions of different frequencies, so they will typically range from -1 to 1. The exact values will depend on the position of the token in the sequence and the dimension of the vector.

<hr class="__chatgpt_plugin">

role::user

